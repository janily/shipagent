//! Reverse-RPC handlers. Native dialogs/actions are provided by the application.
use crate::*;
use agentkit_protocol::RpcValue;
use futures::{FutureExt, future::BoxFuture};
/// Async reverse-RPC handler. Futures must not block; binary leaves are native Bytes.
/// Handlers may forward to a GPUI foreground task; no native action is enabled by default.
pub type CapabilityHandler =
    Arc<dyn Fn(Vec<RpcValue>) -> BoxFuture<'static, Result<RpcValue, WireError>> + Send + Sync>;
pub(crate) type Handlers = std::collections::BTreeMap<String, CapabilityHandler>;
pub(crate) fn permitted(name: &str) -> bool {
    name != "client:browser:invoke"
}

pub(crate) fn failure(
    frame: &Envelope,
    code: agentkit_protocol::ErrorCode,
    message: &str,
) -> Envelope {
    Envelope {
        id: frame.id.clone(),
        channel: frame.channel.clone(),
        kind: EnvelopeKind::Error,
        error: Some(WireError {
            code,
            message: message.into(),
            data: None,
        }),
        ..Envelope::default()
    }
}
/// Owned by one socket generation. Dropping cancels outstanding async handlers.
pub(crate) struct Dispatcher {
    tasks: Vec<smol::Task<()>>,
    send: async_channel::Sender<Envelope>,
    pub replies: async_channel::Receiver<Envelope>,
    timeout: Duration,
}
impl Dispatcher {
    pub fn new(timeout: Duration) -> Self {
        let (send, replies) = async_channel::unbounded();
        Self {
            tasks: Vec::new(),
            send,
            replies,
            timeout,
        }
    }
    pub fn start(&mut self, frame: Envelope, shared: &Shared, advertised: &[String]) {
        self.tasks.retain(|task| !task.is_finished());
        let name = frame.channel.as_deref().unwrap_or("");
        let handler = if permitted(name) && advertised.iter().any(|v| v == name) {
            locked(&shared.capabilities).get(name).cloned()
        } else {
            None
        };
        let Some(handler) = handler else {
            let _ = self.send.try_send(failure(
                &frame,
                agentkit_protocol::ErrorCode::CapabilityUnavailable,
                "Client capability unavailable",
            ));
            return;
        };
        if self.tasks.len() >= 64 {
            let _ = self.send.try_send(failure(
                &frame,
                agentkit_protocol::ErrorCode::CapabilityUnavailable,
                "Client capability busy",
            ));
            return;
        }
        let send = self.send.clone();
        let timeout = self.timeout;
        self.tasks.push(smol::spawn(async move {
            // Both handler construction and polling run outside the socket thread.
            let invoke = std::panic::AssertUnwindSafe(async {
                let args = frame
                    .args
                    .clone()
                    .unwrap_or_default()
                    .into_iter()
                    .map(serde_json::from_value)
                    .collect::<Result<Vec<RpcValue>, _>>()
                    .map_err(|_| WireError {
                        code: agentkit_protocol::ErrorCode::HandlerError,
                        message: "Invalid capability arguments".into(),
                        data: None,
                    })?;
                handler(args).await
            })
            .catch_unwind();
            let result = smol::future::race(
                async {
                    invoke.await.unwrap_or_else(|_| {
                        Err(WireError {
                            code: agentkit_protocol::ErrorCode::HandlerError,
                            message: "Client capability failed".into(),
                            data: None,
                        })
                    })
                },
                async {
                    smol::Timer::after(timeout).await;
                    Err(WireError {
                        code: agentkit_protocol::ErrorCode::ClientRequestTimeout,
                        message: "Client capability timed out".into(),
                        data: None,
                    })
                },
            )
            .await;
            let reply = match result {
                Ok(value) => match serde_json::to_value(value) {
                    Ok(value) => Envelope {
                        id: frame.id.clone(),
                        channel: frame.channel.clone(),
                        kind: EnvelopeKind::Response,
                        result: Some(value),
                        ..Envelope::default()
                    },
                    Err(_) => failure(
                        &frame,
                        agentkit_protocol::ErrorCode::HandlerError,
                        "Invalid capability result",
                    ),
                },
                Err(error) => Envelope {
                    id: frame.id,
                    channel: frame.channel,
                    kind: EnvelopeKind::Error,
                    error: Some(error),
                    ..Envelope::default()
                },
            };
            let _ = send.send(reply).await;
        }));
    }
}
