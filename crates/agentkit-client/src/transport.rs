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

pub(super) async fn run(
    options: ClientOptions,
    commands: async_channel::Receiver<Command>,
    subscribers: Subscribers,
    state: Arc<Mutex<ConnectionState>>,
) {
    let mut pending = HashMap::<String, Pending>::new();
    if let Ok((mut socket, ack)) = timed(connect(&options, &state), options.connect_timeout).await {
        let channels = ack.registered_channels;
        let max_size = ack.max_message_bytes;
        let mut last_seq = 0;
        let mut last_ack = Instant::now();
        enum Input {
            Command(Result<Command, async_channel::RecvError>),
            Frame(Option<Result<Message, async_tungstenite::tungstenite::Error>>),
            Tick,
        }
        loop {
            let input = smol::future::race(
                smol::future::race(
                    commands.recv().map(Input::Command),
                    socket.next().map(Input::Frame),
                ),
                async {
                    smol::Timer::after(Duration::from_millis(10)).await;
                    Input::Tick
                },
            )
            .await;
            match input {
                Input::Command(Ok(Command::Invoke {
                    frame,
                    deadline,
                    result,
                })) => {
                    if deadline <= Instant::now() {
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
                    let id = frame.id.clone();
                    pending.insert(id, Pending { deadline, result });
                    if socket.send(Message::Text(encoded.into())).await.is_err() {
                        break;
                    }
                }
                Input::Command(Ok(Command::Close)) | Input::Command(Err(_)) => {
                    let _ = socket.close(None).await;
                    break;
                }
                Input::Frame(Some(Ok(Message::Text(text)))) => {
                    let Ok(frame) = serde_json::from_str::<Envelope>(&text) else {
                        break;
                    };
                    match frame.kind {
                        EnvelopeKind::Response | EnvelopeKind::Error => {
                            if let Some(request) = pending.remove(&frame.id) {
                                let result = if let Some(error) = frame.error {
                                    Err(RpcError::Wire(error))
                                } else {
                                    Ok(frame.result.unwrap_or(Value::Null))
                                };
                                let _ = request.result.send(result);
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
                            if send(&mut socket, &reply).await.is_err() {
                                break;
                            }
                        }
                        _ => {}
                    }
                }
                Input::Frame(Some(Ok(Message::Ping(_)))) => {
                    if socket.flush().await.is_err() {
                        break;
                    }
                }
                Input::Frame(Some(Ok(Message::Close(_))))
                | Input::Frame(None)
                | Input::Frame(Some(Err(_))) => break,
                _ => {}
            }
            let now = Instant::now();
            let expired: Vec<_> = pending
                .iter()
                .filter(|(_, p)| p.deadline <= now)
                .map(|(id, _)| id.clone())
                .collect();
            for id in expired {
                if let Some(request) = pending.remove(&id) {
                    let _ = request.result.send(Err(RpcError::Timeout));
                }
            }
            if now.duration_since(last_ack) >= Duration::from_secs(5) {
                let ack = Envelope {
                    kind: EnvelopeKind::SequenceAck,
                    last_seq: Some(last_seq),
                    ..Envelope::default()
                };
                if send(&mut socket, &ack).await.is_err() {
                    break;
                }
                last_ack = now;
            }
        }
    }
    *locked(&state) = ConnectionState::Disconnected;
    for (_, request) in pending {
        let _ = request.result.send(Err(RpcError::Disconnected));
    }
    locked(&subscribers).clear();
}
