#![allow(dead_code)]
use async_tungstenite::{WebSocketStream, tungstenite::Message};
use futures::StreamExt;
use serde_json::Value;
use std::time::Duration;
pub type Socket = WebSocketStream<async_net::TcpStream>;
#[allow(clippy::expect_used)]
pub async fn frame(ws: &mut Socket) -> Value {
    loop {
        if let Message::Text(text) = bounded(ws.next())
            .await
            .expect("frame deadline")
            .expect("socket closed")
            .expect("socket IO")
        {
            return serde_json::from_str(&text).unwrap();
        }
    }
}
pub async fn send(ws: &mut Socket, value: Value) {
    ws.send(Message::Text(value.to_string().into()))
        .await
        .unwrap();
}
pub async fn bounded<T>(future: impl std::future::Future<Output = T>) -> Option<T> {
    smol::future::race(async { Some(future.await) }, async {
        smol::Timer::after(Duration::from_secs(4)).await;
        None
    })
    .await
}
