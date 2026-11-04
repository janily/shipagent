use super::{Action, failure, unavailable};
use agentkit_client::CapabilityHandler;
use agentkit_protocol::{RpcValue, WireError};
use std::{
    collections::BTreeMap,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};
pub(super) type Reply = Result<RpcValue, WireError>;
struct Lease {
    alive: Arc<AtomicBool>,
    cancel: async_channel::Sender<()>,
}
impl Drop for Lease {
    fn drop(&mut self) {
        self.alive.store(false, Ordering::Release);
        let _ = self.cancel.try_send(());
    }
}
pub(super) struct Request {
    pub action: Action,
    pub alive: Arc<AtomicBool>,
    pub canceled: async_channel::Receiver<()>,
    pub reply: async_channel::Sender<Reply>,
}
impl Request {
    pub fn live(&self) -> bool {
        self.alive.load(Ordering::Acquire)
    }
    pub fn finish(&self, reply: Reply) {
        if self.live() {
            let _ = self.reply.try_send(reply);
        }
    }
}
pub(super) fn channel() -> (
    BTreeMap<String, CapabilityHandler>,
    async_channel::Receiver<Request>,
) {
    let (tx, rx) = async_channel::bounded(64);
    let handlers = super::NAMES
        .into_iter()
        .map(|name| {
            let tx = tx.clone();
            let handler: CapabilityHandler = Arc::new(move |args| {
                let tx = tx.clone();
                Box::pin(async move {
                    let action = super::parse(name, args)?;
                    let alive = Arc::new(AtomicBool::new(true));
                    let (cancel, canceled) = async_channel::bounded(1);
                    let _lease = Lease {
                        alive: alive.clone(),
                        cancel,
                    };
                    let (reply, response) = async_channel::bounded(1);
                    tx.try_send(Request {
                        action,
                        alive,
                        canceled,
                        reply,
                    })
                    .map_err(|_| unavailable("Native capability queue unavailable"))?;
                    response
                        .recv()
                        .await
                        .map_err(|_| failure("Native capability canceled"))?
                })
            });
            (name.into(), handler)
        })
        .collect();
    (handlers, rx)
}
