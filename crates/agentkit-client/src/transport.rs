//! Single-owner socket/request loop; no frontend thread performs network IO.
use super::*;
use async_tungstenite::{WebSocketStream, tungstenite::Message};
use futures::{FutureExt, SinkExt, StreamExt};
use secrecy::ExposeSecret;
type Socket = WebSocketStream<async_net::TcpStream>;
struct Pending {
    deadline: Instant,
    result: oneshot::Sender<Result<Value, RpcError>>,
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
async fn send(socket: &mut Socket, frame: &Envelope) -> Result<(), RpcError> {
    socket
        .send(Message::Text(serde_json::to_string(frame)?.into()))
        .await
        .map_err(|e| RpcError::Transport(e.to_string()))
}
async fn connect(
    options: &ClientOptions,
    state: &Mutex<ConnectionState>,
) -> Result<(Socket, Envelope), RpcError> {
    let url =
        url::Url::parse(&options.endpoint.url).map_err(|e| RpcError::Transport(e.to_string()))?;
    if url.scheme() != "ws"
        || !matches!(url.host_str(), Some("127.0.0.1" | "localhost" | "[::1]"))
        || !url.username().is_empty()
        || url.password().is_some()
    {
        return Err(RpcError::Transport(
            "Prototype supports loopback ws endpoints only".into(),
        ));
    }
    let host = url
        .host_str()
        .ok_or_else(|| RpcError::Transport("Missing host".into()))?
        .trim_matches(['[', ']']);
    let port = url
        .port_or_known_default()
        .ok_or_else(|| RpcError::Transport("Missing port".into()))?;
    let stream = async_net::TcpStream::connect((host, port))
        .await
        .map_err(|e| RpcError::Transport(e.to_string()))?;
    let (mut socket, _) = async_tungstenite::client_async(options.endpoint.url.as_str(), stream)
        .await
        .map_err(|e| RpcError::Transport(e.to_string()))?;
    *locked(state) = ConnectionState::Handshaking;
    let handshake = Envelope {
        id: uuid::Uuid::new_v4().to_string(),
        kind: EnvelopeKind::Handshake,
        protocol_version: Some(agentkit_protocol::PROTOCOL_VERSION.into()),
        token: Some(options.endpoint.token.expose_secret().into()),
        workspace_id: Some(options.workspace_id.clone()),
        client_capabilities: Some(vec![]),
        ..Envelope::default()
    };
    send(&mut socket, &handshake).await?;
    while let Some(frame) = socket.next().await {
        match frame.map_err(|e| RpcError::Transport(e.to_string()))? {
            Message::Text(text) => {
                let ack: Envelope = serde_json::from_str(&text)?;
                if ack.kind == EnvelopeKind::Error {
                    return Err(ack
                        .error
                        .map(RpcError::Wire)
                        .unwrap_or(RpcError::Disconnected));
                }
                if ack.kind != EnvelopeKind::HandshakeAck {
                    return Err(RpcError::Transport("Expected handshake_ack".into()));
                }
                if ack
                    .protocol_version
                    .as_deref()
                    .is_some_and(|v| v != agentkit_protocol::PROTOCOL_VERSION)
                {
                    return Err(RpcError::Transport("Unsupported protocol version".into()));
                }
                *locked(state) = ConnectionState::Connected;
                return Ok((socket, ack));
            }
            Message::Ping(_) => socket
                .flush()
                .await
                .map_err(|e| RpcError::Transport(e.to_string()))?,
            Message::Close(_) => return Err(RpcError::Disconnected),
            _ => {}
        }
    }
    Err(RpcError::Disconnected)
}

async fn controlled<T>(
    future: impl std::future::Future<Output = Result<T, RpcError>>,
    duration: Duration,
    stop: &async_channel::Receiver<()>,
) -> Result<T, RpcError> {
    smol::future::race(timed(future, duration), async {
        let _ = stop.recv().await;
        Err(RpcError::Disconnected)
    })
    .await
}
fn io_deadline(pending: &HashMap<String, Pending>) -> Duration {
    pending
        .values()
        .map(|p| p.deadline.saturating_duration_since(Instant::now()))
        .min()
        .unwrap_or(Duration::from_secs(5))
        .min(Duration::from_secs(5))
}
fn expire(pending: &mut HashMap<String, Pending>) {
    let now = Instant::now();
    let expired: Vec<_> = pending
        .iter()
        .filter(|(_, p)| p.deadline <= now || p.result.is_canceled())
        .map(|(id, _)| id.clone())
        .collect();
    for id in expired {
        if let Some(request) = pending.remove(&id) {
            let _ = request.result.send(Err(RpcError::Timeout));
        }
    }
}

pub(super) async fn run(
    options: ClientOptions,
    commands: async_channel::Receiver<Command>,
    subscribers: Subscribers,
    state: Arc<Mutex<ConnectionState>>,
    stop: async_channel::Receiver<()>,
    failure: Arc<Mutex<Option<RpcError>>>,
) {
    let mut pending = HashMap::<String, Pending>::new();
    let outcome = controlled(connect(&options, &state), options.connect_timeout, &stop).await;
    let reason = match outcome {
        Err(error) => error,
        Ok((mut socket, ack)) => {
            let channels = ack.registered_channels;
            let max_size = ack.max_message_bytes;
            let mut last_seq = 0;
            let mut last_ack = Instant::now();
            enum Input {
                Command(Result<Command, async_channel::RecvError>),
                Frame(Option<Result<Message, async_tungstenite::tungstenite::Error>>),
                Tick,
                Stop,
            }
            loop {
                // Run maintenance before any branch can reject a locally invalid call.
                expire(&mut pending);
                if last_ack.elapsed() >= Duration::from_secs(5) {
                    let ack = Envelope {
                        id: uuid::Uuid::new_v4().to_string(),
                        kind: EnvelopeKind::SequenceAck,
                        last_seq: Some(last_seq),
                        ..Envelope::default()
                    };
                    if let Err(error) =
                        controlled(send(&mut socket, &ack), io_deadline(&pending), &stop).await
                    {
                        break error;
                    }
                    last_ack = Instant::now();
                }
                let input = smol::future::race(
                    smol::future::race(
                        commands.recv().map(Input::Command),
                        socket.next().map(Input::Frame),
                    ),
                    smol::future::race(
                        async {
                            smol::Timer::after(Duration::from_millis(10)).await;
                            Input::Tick
                        },
                        async {
                            let _ = stop.recv().await;
                            Input::Stop
                        },
                    ),
                )
                .await;
                match input {
                    Input::Command(Ok(Command::Invoke {
                        frame,
                        deadline,
                        result,
                    })) => {
                        if deadline <= Instant::now() || result.is_canceled() {
                            let _ = result.send(Err(RpcError::Timeout));
                            continue;
                        }
                        if channels
                            .as_ref()
                            .is_some_and(|c| !c.iter().any(|c| Some(c) == frame.channel.as_ref()))
                        {
                            let _ = result.send(Err(RpcError::ChannelUnavailable(
                                frame.channel.unwrap_or_default(),
                            )));
                            continue;
                        }
                        let encoded = match serde_json::to_string(&frame) {
                            Ok(v) => v,
                            Err(e) => {
                                let _ = result.send(Err(e.into()));
                                continue;
                            }
                        };
                        if max_size.is_some_and(|limit| encoded.len() as u64 > limit) {
                            let _ = result.send(Err(RpcError::TooLarge));
                            continue;
                        }
                        // Serialization can itself outlast a short deadline. Never send that stale write.
                        if deadline <= Instant::now() || result.is_canceled() {
                            let _ = result.send(Err(RpcError::Timeout));
                            continue;
                        }
                        pending.insert(frame.id.clone(), Pending { deadline, result });
                        let write = async {
                            socket
                                .send(Message::Text(encoded.into()))
                                .await
                                .map_err(|e| RpcError::Transport(e.to_string()))
                        };
                        if let Err(error) = controlled(write, io_deadline(&pending), &stop).await {
                            break error;
                        }
                    }
                    Input::Command(Err(_)) | Input::Stop => {
                        let _ = timed(
                            async {
                                socket
                                    .close(None)
                                    .await
                                    .map_err(|e| RpcError::Transport(e.to_string()))
                            },
                            Duration::from_millis(100),
                        )
                        .await;
                        break RpcError::Disconnected;
                    }
                    Input::Frame(Some(Ok(Message::Text(text)))) => {
                        let frame: Envelope = match serde_json::from_str(&text) {
                            Ok(frame) => frame,
                            Err(e) => break e.into(),
                        };
                        match frame.kind {
                            EnvelopeKind::Response | EnvelopeKind::Error => {
                                if let Some(request) = pending.remove(&frame.id) {
                                    let reply = frame.error.map_or_else(
                                        || Ok(frame.result.unwrap_or(Value::Null)),
                                        |e| Err(RpcError::Wire(e)),
                                    );
                                    let _ = request.result.send(reply);
                                }
                            }
                            EnvelopeKind::Event => {
                                if let Some(seq) = frame.seq {
                                    last_seq = last_seq.max(seq);
                                }
                                if let Some(channel) = frame.channel {
                                    let mut table = locked(&subscribers);
                                    if let Some(list) = table.get_mut(&channel) {
                                        let args = frame.args.unwrap_or_default();
                                        list.retain(|s| s.try_send(args.clone()).is_ok());
                                    }
                                }
                            }
                            EnvelopeKind::Request => {
                                let reply = Envelope {
                                    id: frame.id,
                                    kind: EnvelopeKind::Error,
                                    error: Some(WireError {
                                        code: agentkit_protocol::ErrorCode::CapabilityUnavailable,
                                        message: "Client capability unavailable".into(),
                                        data: None,
                                    }),
                                    ..Envelope::default()
                                };
                                if let Err(error) = controlled(
                                    send(&mut socket, &reply),
                                    io_deadline(&pending),
                                    &stop,
                                )
                                .await
                                {
                                    break error;
                                }
                            }
                            _ => {}
                        }
                    }
                    Input::Frame(Some(Ok(Message::Ping(_)))) => {
                        let flush = async {
                            socket
                                .flush()
                                .await
                                .map_err(|e| RpcError::Transport(e.to_string()))
                        };
                        if let Err(error) = controlled(flush, io_deadline(&pending), &stop).await {
                            break error;
                        }
                    }
                    Input::Frame(Some(Ok(Message::Close(_)))) | Input::Frame(None) => {
                        break RpcError::Disconnected;
                    }
                    Input::Frame(Some(Err(error))) => break RpcError::Transport(error.to_string()),
                    _ => {}
                }
            }
        }
    };
    *locked(&failure) = Some(reason.clone());
    *locked(&state) = ConnectionState::Disconnected;
    commands.close();
    for (_, request) in pending {
        let _ = request.result.send(Err(reason.clone()));
    }
    // async-channel retains queued messages while senders live: explicitly reject them.
    while let Ok(Command::Invoke { result, .. }) = commands.try_recv() {
        let _ = result.send(Err(reason.clone()));
    }
    locked(&subscribers).clear();
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn actual_blocked_socket_write_obeys_controlled_deadline() {
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
            let result = controlled(
                async {
                    socket
                        .send(payload)
                        .await
                        .map_err(|e| RpcError::Transport(e.to_string()))
                },
                Duration::from_millis(100),
                &stopped,
            )
            .await;
            assert!(matches!(result, Err(RpcError::Timeout)));
            assert!(start.elapsed() < Duration::from_secs(2));
            // Actual frame bytes reached TCP. Both buffer bounds ensure the
            // complete 8 MiB frame cannot have been accepted by the sender.
            let mut byte = [0];
            receiver.read_exact(&mut byte).unwrap();
        });
    }
}
