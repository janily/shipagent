//! Single-owner transport, reconnect cursor and IO deadlines.
use super::*;
use async_tungstenite::{
    WebSocketStream,
    tungstenite::{Message, protocol::WebSocketConfig},
};
use futures::{FutureExt, SinkExt, StreamExt};
use secrecy::ExposeSecret;
type Socket = WebSocketStream<async_net::TcpStream>;
struct Pending {
    deadline: Instant,
    result: oneshot::Sender<Result<Value, RpcError>>,
    switch_to: Option<String>,
}
#[derive(Debug)]
enum Exit {
    Lost(RpcError),
    Replace(Endpoint),
    Closed,
}
#[derive(Default)]
struct Cursor {
    client_id: Option<String>,
    seq: u64,
    server_id: Option<String>,
}
#[derive(Default)]
struct Backoff {
    attempt: u32,
}
impl Backoff {
    fn next(&mut self, cap: Duration) -> Duration {
        let delay = Duration::from_millis(
            1000_u64.saturating_mul(1_u64.checked_shl(self.attempt).unwrap_or(u64::MAX)),
        )
        .min(cap);
        self.attempt = self.attempt.saturating_add(1);
        delay
    }
    fn stable(&mut self, lived: Duration) {
        if lived >= Duration::from_secs(10) {
            self.attempt = 0;
        }
    }
}
async fn timed<T>(
    future: impl std::future::Future<Output = Result<T, RpcError>>,
    duration: Duration,
) -> Result<T, RpcError> {
    smol::future::race(future, async {
        smol::Timer::after(duration).await;
        Err(RpcError::Timeout)
    })
    .await
}
async fn io<T>(
    future: impl std::future::Future<Output = Result<T, RpcError>>,
    duration: Duration,
    stop: &async_channel::Receiver<()>,
    updates: &async_channel::Receiver<Endpoint>,
) -> Result<T, Exit> {
    smol::future::race(
        timed(future, duration).map(|r| r.map_err(Exit::Lost)),
        smol::future::race(
            async {
                let _ = stop.recv().await;
                Err(Exit::Closed)
            },
            async {
                match updates.recv().await {
                    Ok(ep) => Err(Exit::Replace(ep)),
                    Err(_) => {
                        std::future::pending::<()>().await;
                        Err(Exit::Closed)
                    }
                }
            },
        ),
    )
    .await
}
fn transport(error: impl std::fmt::Display) -> RpcError {
    RpcError::Transport(error.to_string())
}
async fn send(socket: &mut Socket, frame: &Envelope) -> Result<(), RpcError> {
    socket
        .send(Message::Text(serde_json::to_string(frame)?.into()))
        .await
        .map_err(transport)
}
async fn connect(
    options: &ClientOptions,
    shared: &Shared,
    cursor: &Cursor,
) -> Result<(Socket, Envelope, Vec<String>), RpcError> {
    let url = url::Url::parse(&options.endpoint.url).map_err(|_| RpcError::Configuration)?;
    if url.scheme() != "ws"
        || !matches!(url.host_str(), Some("127.0.0.1" | "localhost" | "[::1]"))
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err(RpcError::Configuration);
    }
    let host = url
        .host_str()
        .ok_or(RpcError::Configuration)?
        .trim_matches(['[', ']']);
    let port = url.port_or_known_default().ok_or(RpcError::Configuration)?;
    let tcp = async_net::TcpStream::connect((host, port))
        .await
        .map_err(transport)?;
    let limit = agentkit_protocol::constants::limits::DEFAULT_MAX_MESSAGE_BYTES as usize;
    let config = WebSocketConfig::default()
        .max_message_size(Some(limit))
        .max_frame_size(Some(limit));
    let (mut socket, _) = async_tungstenite::client_async_with_config(
        options.endpoint.url.as_str(),
        tcp,
        Some(config),
    )
    .await
    .map_err(transport)?;
    shared.set_state(ConnectionState::Handshaking);
    let advertised: Vec<String> = locked(&shared.capabilities)
        .keys()
        .filter(|name| crate::capability::permitted(name))
        .cloned()
        .collect();
    send(
        &mut socket,
        &Envelope {
            id: uuid::Uuid::new_v4().to_string(),
            kind: EnvelopeKind::Handshake,
            protocol_version: Some(agentkit_protocol::PROTOCOL_VERSION.into()),
            token: Some(options.endpoint.token.expose_secret().into()),
            workspace_id: Some(options.workspace_id.clone()),
            client_capabilities: Some(advertised.clone()),
            reconnect_client_id: cursor.client_id.clone(),
            last_seq: cursor.client_id.as_ref().map(|_| cursor.seq),
            ..Envelope::default()
        },
    )
    .await?;
    while let Some(message) = socket.next().await {
        match message.map_err(transport)? {
            Message::Text(text) => {
                let ack: Envelope = serde_json::from_str(&text)?;
                if ack.kind == EnvelopeKind::Error {
                    return Err(ack
                        .error
                        .map(RpcError::Wire)
                        .unwrap_or(RpcError::Disconnected));
                }
                if ack.kind != EnvelopeKind::HandshakeAck {
                    return Err(RpcError::Configuration);
                }
                if ack
                    .protocol_version
                    .as_deref()
                    .is_some_and(|v| v != agentkit_protocol::PROTOCOL_VERSION)
                {
                    return Err(RpcError::Wire(WireError {
                        code: agentkit_protocol::ErrorCode::ProtocolVersionUnsupported,
                        message: "Unsupported protocol version".into(),
                        data: None,
                    }));
                }
                if ack.max_message_bytes == Some(0) {
                    return Err(RpcError::Configuration);
                }
                return Ok((socket, ack, advertised));
            }
            Message::Ping(_) => socket.flush().await.map_err(transport)?,
            Message::Close(_) => return Err(RpcError::Disconnected),
            _ => {}
        }
    }
    Err(RpcError::Disconnected)
}
fn expire(pending: &mut HashMap<String, Pending>) {
    let now = Instant::now();
    let expired: Vec<_> = pending
        .iter()
        .filter(|(_, p)| p.deadline <= now || p.result.is_canceled())
        .map(|(id, _)| id.clone())
        .collect();
    for id in expired {
        if let Some(p) = pending.remove(&id) {
            let _ = p.result.send(Err(RpcError::Timeout));
        }
    }
}
fn io_deadline(pending: &HashMap<String, Pending>) -> Duration {
    pending
        .values()
        .map(|p| p.deadline.saturating_duration_since(Instant::now()))
        .min()
        .unwrap_or(Duration::from_secs(5))
        .min(Duration::from_secs(5))
}
fn invalidate(shared: &Shared, commands: &async_channel::Receiver<Command>, reason: RpcError) {
    let mut info = locked(&shared.info);
    info.generation = info.generation.wrapping_add(1);
    info.failure = Some(reason.clone());
    info.snapshot.registered_channels = None;
    info.snapshot.server_version = None;
    while let Ok(Command::Invoke { result, .. }) = commands.try_recv() {
        let _ = result.send(Err(reason.clone()));
    }
}
fn terminal(error: &RpcError) -> bool {
    matches!(error, RpcError::Configuration)
        || matches!(error,RpcError::Wire(e) if matches!(e.code,agentkit_protocol::ErrorCode::AuthFailed|agentkit_protocol::ErrorCode::ProtocolVersionUnsupported))
}
fn validate_binary(value: &Value) -> Result<(), RpcError> {
    match value {
        Value::Object(map) if map.get("__craftRpcType").and_then(Value::as_str) == Some("u8") => {
            let _: agentkit_protocol::RpcValue = serde_json::from_value(value.clone())?;
        }
        Value::Object(map) => {
            for value in map.values() {
                validate_binary(value)?;
            }
        }
        Value::Array(array) => {
            for value in array {
                validate_binary(value)?;
            }
        }
        _ => {}
    }
    Ok(())
}

struct SessionContext<'a> {
    options: &'a mut ClientOptions,
    shared: &'a Shared,
    commands: &'a async_channel::Receiver<Command>,
    stop: &'a async_channel::Receiver<()>,
    updates: &'a async_channel::Receiver<Endpoint>,
    cursor: &'a mut Cursor,
    advertised: &'a [String],
}
async fn session(socket: &mut Socket, ack: &Envelope, context: SessionContext<'_>) -> Exit {
    let SessionContext {
        options,
        shared,
        commands,
        stop,
        updates,
        cursor,
        advertised,
    } = context;
    let mut pending = HashMap::<String, Pending>::new();
    let mut last_ack = Instant::now();
    let mut dispatcher = crate::capability::Dispatcher::new(options.capability_timeout);
    let limit = ack
        .max_message_bytes
        .unwrap_or(agentkit_protocol::constants::limits::DEFAULT_MAX_MESSAGE_BYTES);
    let outcome = async {
        enum Input {
            Command(Result<Command, async_channel::RecvError>),
            Frame(Option<Result<Message, async_tungstenite::tungstenite::Error>>),
            Reply(Result<Box<Envelope>, async_channel::RecvError>),
            Tick,
        }
        loop {
            expire(&mut pending);
            if last_ack.elapsed() >= Duration::from_secs(5) {
                io(
                    send(
                        socket,
                        &Envelope {
                            id: uuid::Uuid::new_v4().to_string(),
                            kind: EnvelopeKind::SequenceAck,
                            last_seq: Some(cursor.seq),
                            ..Envelope::default()
                        },
                    ),
                    io_deadline(&pending),
                    stop,
                    updates,
                )
                .await?;
                last_ack = Instant::now();
            }
            let next = smol::future::race(
                smol::future::race(
                    commands.recv().map(Input::Command),
                    socket.next().map(Input::Frame),
                ),
                smol::future::race(
                    dispatcher
                        .replies
                        .recv()
                        .map(|r| Input::Reply(r.map(Box::new))),
                    async {
                        smol::Timer::after(Duration::from_millis(10)).await;
                        Input::Tick
                    },
                ),
            );
            let input = io(
                async { Ok(next.await) },
                Duration::from_secs(60),
                stop,
                updates,
            )
            .await?;
            match input {
                Input::Command(Ok(Command::Invoke {
                    frame,
                    deadline,
                    result,
                    generation,
                    switch_to,
                })) => {
                    if generation != locked(&shared.info).generation {
                        let _ = result.send(Err(RpcError::Disconnected));
                        continue;
                    }
                    if deadline <= Instant::now() || result.is_canceled() {
                        let _ = result.send(Err(RpcError::Timeout));
                        continue;
                    }
                    if ack
                        .registered_channels
                        .as_ref()
                        .is_some_and(|c| !c.iter().any(|c| Some(c) == frame.channel.as_ref()))
                    {
                        let _ = result.send(Err(RpcError::ChannelUnavailable(
                            frame.channel.unwrap_or_default(),
                        )));
                        continue;
                    }
                    if let Some(args) = &frame.args {
                        let validation = args.iter().try_for_each(validate_binary);
                        if let Err(error) = validation {
                            let _ = result.send(Err(error));
                            continue;
                        }
                    }
                    let encoded = match serde_json::to_string(&frame) {
                        Ok(v) => v,
                        Err(e) => {
                            let _ = result.send(Err(e.into()));
                            continue;
                        }
                    };
                    if encoded.len() as u64 > limit {
                        let _ = result.send(Err(RpcError::TooLarge));
                        continue;
                    }
                    if deadline <= Instant::now() || result.is_canceled() {
                        let _ = result.send(Err(RpcError::Timeout));
                        continue;
                    }
                    pending.insert(
                        frame.id.clone(),
                        Pending {
                            deadline,
                            result,
                            switch_to,
                        },
                    );
                    io(
                        async {
                            socket
                                .send(Message::Text(encoded.into()))
                                .await
                                .map_err(transport)
                        },
                        io_deadline(&pending),
                        stop,
                        updates,
                    )
                    .await?;
                }
                Input::Command(Err(_)) => return Err(Exit::Closed),
                Input::Frame(Some(Ok(Message::Text(text)))) => {
                    let frame: Envelope =
                        serde_json::from_str(&text).map_err(|e| Exit::Lost(e.into()))?;
                    if let Some(value) = &frame.result {
                        validate_binary(value).map_err(Exit::Lost)?;
                    }
                    if let Some(args) = &frame.args {
                        for value in args {
                            validate_binary(value).map_err(Exit::Lost)?;
                        }
                    }
                    match frame.kind {
                        EnvelopeKind::Response | EnvelopeKind::Error => {
                            if let Some(p) = pending.remove(&frame.id) {
                                if frame.error.is_none()
                                    && let Some(workspace) = p.switch_to
                                {
                                    options.workspace_id = workspace;
                                }
                                let reply = frame.error.map_or_else(
                                    || Ok(frame.result.unwrap_or(Value::Null)),
                                    |e| Err(RpcError::Wire(e)),
                                );
                                let _ = p.result.send(reply);
                            }
                        }
                        EnvelopeKind::Event => {
                            if let Some(seq) = frame.seq {
                                if cursor.seq > 0 && seq > cursor.seq.saturating_add(1) {
                                    tracing::warn!(
                                        expected = cursor.seq.saturating_add(1),
                                        received = seq,
                                        "RPC event sequence gap"
                                    );
                                }
                                cursor.seq = cursor.seq.max(seq);
                                locked(&shared.info).snapshot.last_seen_seq = cursor.seq;
                            }
                            if let Some(channel) = frame.channel {
                                shared.publish(&channel, frame.args.unwrap_or_default());
                            }
                        }
                        EnvelopeKind::Request => dispatcher.start(frame, shared, advertised),
                        _ => {}
                    }
                }
                Input::Reply(Ok(mut reply)) => {
                    let mut encoded =
                        serde_json::to_string(&reply).map_err(|e| Exit::Lost(e.into()))?;
                    if encoded.len() as u64 > limit {
                        *reply = crate::capability::failure(
                            &reply,
                            agentkit_protocol::ErrorCode::TransferTooLarge,
                            "Client capability reply exceeds frame limit",
                        );
                        encoded =
                            serde_json::to_string(&reply).map_err(|e| Exit::Lost(e.into()))?;
                    }
                    if encoded.len() as u64 > limit {
                        return Err(Exit::Lost(RpcError::TooLarge));
                    }
                    io(
                        async {
                            socket
                                .send(Message::Text(encoded.into()))
                                .await
                                .map_err(transport)
                        },
                        io_deadline(&pending),
                        stop,
                        updates,
                    )
                    .await?;
                }
                Input::Frame(Some(Ok(Message::Ping(_)))) => {
                    io(
                        async { socket.flush().await.map_err(transport) },
                        io_deadline(&pending),
                        stop,
                        updates,
                    )
                    .await?
                }
                Input::Frame(Some(Ok(Message::Close(_)))) | Input::Frame(None) => {
                    return Err(Exit::Lost(RpcError::Disconnected));
                }
                Input::Frame(Some(Err(e))) => return Err(Exit::Lost(transport(e))),
                _ => {}
            }
        }
    }
    .await;
    // Invalidate queued calls before waking any sent caller; a newly queued call
    // belongs to the next generation even when it wakes immediately on loss.
    let exit = match outcome {
        Err(exit) => exit,
        Ok::<(), Exit>(()) => Exit::Closed,
    };
    invalidate(shared, commands, RpcError::Disconnected);
    expire(&mut pending);
    for (_, p) in pending {
        let _ = p.result.send(Err(RpcError::Disconnected));
    }
    exit
}
pub(super) async fn run(
    mut options: ClientOptions,
    commands: async_channel::Receiver<Command>,
    shared: Arc<Shared>,
    stop: async_channel::Receiver<()>,
    updates: async_channel::Receiver<Endpoint>,
) {
    let mut cursor = Cursor::default();
    let mut backoff = Backoff::default();
    let mut had_connection = false;
    loop {
        if stop.is_closed() || !stop.is_empty() {
            break;
        }
        shared.set_state(ConnectionState::Connecting);
        let outcome = io(
            connect(&options, &shared, &cursor),
            options.connect_timeout,
            &stop,
            &updates,
        )
        .await;
        let (exit, invalidated) = match outcome {
            Ok((mut socket, ack, advertised)) => {
                let resumed = ack.reconnected == Some(true)
                    && !matches!((&cursor.server_id,&ack.server_id),(Some(old),Some(new)) if old!=new);
                if !resumed {
                    cursor.seq = 0;
                }
                cursor.client_id = ack.client_id.clone();
                cursor.server_id = ack.server_id.clone();
                {
                    let mut info = locked(&shared.info);
                    info.failure = None;
                    info.snapshot.client_id = ack.client_id.clone();
                    info.snapshot.server_version = ack.server_version.clone();
                    info.snapshot.registered_channels = ack.registered_channels.clone();
                    info.snapshot.max_message_bytes = ack
                        .max_message_bytes
                        .unwrap_or(agentkit_protocol::constants::limits::DEFAULT_MAX_MESSAGE_BYTES);
                    info.snapshot.last_seen_seq = cursor.seq;
                    info.snapshot.needs_refresh = !resumed || ack.stale == Some(true);
                    info.snapshot.recovery_id += 1;
                }
                shared.set_state(ConnectionState::Connected);
                if had_connection {
                    shared.publish(
                        "__transport:reconnected",
                        vec![serde_json::json!(!resumed || ack.stale == Some(true))],
                    );
                }
                had_connection = true;
                let since = Instant::now();
                let exit = session(
                    &mut socket,
                    &ack,
                    SessionContext {
                        options: &mut options,
                        shared: &shared,
                        commands: &commands,
                        stop: &stop,
                        updates: &updates,
                        cursor: &mut cursor,
                        advertised: &advertised,
                    },
                )
                .await;
                backoff.stable(since.elapsed());
                // Best effort close, bounded independently from blocked writes.
                let _ = timed(
                    async { socket.close(None).await.map_err(transport) },
                    Duration::from_millis(100),
                )
                .await;
                (exit, true)
            }
            Err(exit) => (exit, false),
        };
        match exit {
            Exit::Closed => break,
            Exit::Replace(endpoint) => {
                if !invalidated {
                    invalidate(&shared, &commands, RpcError::Disconnected);
                }
                options.endpoint = endpoint;
                cursor = Cursor::default();
                backoff = Backoff::default();
                continue;
            }
            Exit::Lost(error) => {
                if !invalidated {
                    invalidate(&shared, &commands, error.clone());
                } else {
                    locked(&shared.info).failure = Some(error.clone());
                }
                if terminal(&error) {
                    shared.set_state(ConnectionState::Fatal{reason:if matches!(&error,RpcError::Wire(e) if e.code==agentkit_protocol::ErrorCode::AuthFailed){"Authentication failed".into()}else{"Endpoint or protocol rejected".into()}});
                    // Close the small pre-Fatal enqueue race before waiting for replacement.
                    invalidate(&shared, &commands, error.clone());
                    // A sidecar owner can replace credentials even after a fatal handshake.
                    match smol::future::race(
                        async {
                            let _ = stop.recv().await;
                            None
                        },
                        async { updates.recv().await.ok() },
                    )
                    .await
                    {
                        Some(ep) => {
                            options.endpoint = ep;
                            cursor = Cursor::default();
                            backoff = Backoff::default();
                            continue;
                        }
                        None => break,
                    }
                }
                let delay = backoff.next(options.max_reconnect_delay);
                shared.set_state(ConnectionState::Reconnecting {
                    attempt: backoff.attempt,
                    next_retry_ms: delay.as_millis().min(u64::MAX as u128) as u64,
                });
                match io(
                    async {
                        smol::Timer::after(delay).await;
                        Ok(())
                    },
                    delay + Duration::from_secs(1),
                    &stop,
                    &updates,
                )
                .await
                {
                    Ok(()) => {}
                    Err(Exit::Replace(ep)) => {
                        invalidate(&shared, &commands, RpcError::Disconnected);
                        options.endpoint = ep;
                        cursor = Cursor::default();
                        backoff = Backoff::default();
                    }
                    Err(_) => break,
                }
            }
        }
    }
    let last = locked(&shared.info).failure.clone();
    invalidate(&shared, &commands, RpcError::Disconnected);
    if last.is_some() {
        locked(&shared.info).failure = last;
    }
    commands.close();
    shared.finish();
}
#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn actual_socket_send_completes_or_obeys_deadline() {
        use std::io::Read;
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        socket2::SockRef::from(&listener)
            .set_recv_buffer_size(4096)
            .unwrap();
        let sender = socket2::Socket::new(
            socket2::Domain::IPV4,
            socket2::Type::STREAM,
            Some(socket2::Protocol::TCP),
        )
        .unwrap();
        sender.set_send_buffer_size(4096).unwrap();
        sender
            .connect(&listener.local_addr().unwrap().into())
            .unwrap();
        let sender: std::net::TcpStream = sender.into();
        sender.set_nonblocking(true).unwrap();
        let (mut receiver, _) = listener.accept().unwrap();
        receiver
            .set_read_timeout(Some(Duration::from_secs(1)))
            .unwrap();
        smol::block_on(async {
            let sender = async_net::TcpStream::try_from(sender).unwrap();
            let mut socket = WebSocketStream::from_raw_socket(
                sender,
                async_tungstenite::tungstenite::protocol::Role::Client,
                None,
            )
            .await;
            let payload = Message::Text("x".repeat(8 * 1024 * 1024).into());
            let (_stop, stopped) = async_channel::bounded(1);
            let start = Instant::now();
            let (_endpoints, updates) = async_channel::unbounded();
            let result = io(
                async {
                    socket
                        .send(payload)
                        .await
                        .map_err(|e| RpcError::Transport(e.to_string()))
                },
                Duration::from_millis(100),
                &stopped,
                &updates,
            )
            .await;
            assert!(
                result.is_ok() || matches!(result, Err(Exit::Lost(RpcError::Timeout))),
                "Unexpected TCP write outcome: {result:?}"
            );
            assert!(start.elapsed() < Duration::from_secs(2));
            // Actual frame bytes reached TCP. Some kernels can still accept
            // the complete frame on loopback, even with explicit buffer bounds.
            // The deterministic IO fixture below covers backpressure separately.
            let mut byte = [0];
            receiver.read_exact(&mut byte).unwrap();
            assert_eq!(byte[0], 0x81);
        });
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod backpressure_tests {
    use super::*;
    use std::{
        pin::Pin,
        sync::atomic::{AtomicUsize, Ordering},
        task::{Context, Poll},
    };

    // A real AsyncWrite implementation whose capacity is exhausted after 4KiB.
    // Kernel send buffering cannot defeat this backpressure condition.
    struct BoundedIo {
        capacity: usize,
        written: Arc<AtomicUsize>,
    }
    impl futures::AsyncRead for BoundedIo {
        fn poll_read(
            self: Pin<&mut Self>,
            _: &mut Context<'_>,
            _: &mut [u8],
        ) -> Poll<std::io::Result<usize>> {
            Poll::Pending
        }
    }
    impl futures::AsyncWrite for BoundedIo {
        fn poll_write(
            mut self: Pin<&mut Self>,
            _: &mut Context<'_>,
            bytes: &[u8],
        ) -> Poll<std::io::Result<usize>> {
            let count = bytes.len().min(self.capacity);
            if count == 0 {
                return Poll::Pending;
            }
            self.capacity -= count;
            self.written.fetch_add(count, Ordering::SeqCst);
            Poll::Ready(Ok(count))
        }
        fn poll_flush(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<std::io::Result<()>> {
            Poll::Ready(Ok(()))
        }
        fn poll_close(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<std::io::Result<()>> {
            Poll::Ready(Ok(()))
        }
    }

    #[test]
    fn partially_written_websocket_frame_times_out_under_backpressure() {
        smol::block_on(async {
            let written = Arc::new(AtomicUsize::new(0));
            let stream = BoundedIo {
                capacity: 4096,
                written: written.clone(),
            };
            let mut socket = WebSocketStream::from_raw_socket(
                stream,
                async_tungstenite::tungstenite::protocol::Role::Client,
                None,
            )
            .await;
            let payload = Message::Text("x".repeat(8192).into());
            let (_stop, stopped) = async_channel::bounded(1);
            let start = Instant::now();
            let (_endpoints, updates) = async_channel::unbounded();
            let result = io(
                async {
                    socket
                        .send(payload)
                        .await
                        .map_err(|e| RpcError::Transport(e.to_string()))
                },
                Duration::from_millis(100),
                &stopped,
                &updates,
            )
            .await;
            assert!(matches!(result, Err(Exit::Lost(RpcError::Timeout))));
            assert_eq!(written.load(Ordering::SeqCst), 4096);
            assert!(start.elapsed() < Duration::from_secs(2));
        });
    }
}

#[cfg(test)]
mod backoff_tests {
    use super::*;
    #[test]
    fn exponential_backoff_caps_and_resets_only_after_ten_stable_seconds() {
        let mut b = Backoff::default();
        for seconds in [1, 2, 4, 8, 16, 30, 30] {
            assert_eq!(
                b.next(Duration::from_secs(30)),
                Duration::from_secs(seconds)
            );
        }
        b.stable(Duration::from_millis(9999));
        assert_eq!(b.next(Duration::from_secs(30)), Duration::from_secs(30));
        b.stable(Duration::from_secs(10));
        assert_eq!(b.next(Duration::from_secs(30)), Duration::from_secs(1));
    }
}
