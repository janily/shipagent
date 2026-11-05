#![allow(clippy::unwrap_used)]
use agentkit_client::{ClientOptions, ConnectionState, RpcClient};
use std::time::Duration;
#[test]
fn explicit_retry_wakes_backoff_without_replacing_the_endpoint() {
    smol::block_on(async {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        drop(listener);
        let mut options =
            ClientOptions::local(format!("ws://{address}"), "fixture".into(), "w".into());
        options.connect_timeout = Duration::from_millis(200);
        let client = RpcClient::connect(options);
        let states = client.state_changes();
        loop {
            if matches!(
                states.recv().await.unwrap(),
                ConnectionState::Reconnecting { .. }
            ) {
                break;
            }
        }
        client.retry_now();
        let result = smol::future::race(
            async {
                loop {
                    if matches!(states.recv().await.unwrap(), ConnectionState::Connecting) {
                        break true;
                    }
                }
            },
            async {
                smol::Timer::after(Duration::from_millis(400)).await;
                false
            },
        )
        .await;
        client.close();
        assert!(result, "manual retry must wake the scheduled backoff");
    });
}
