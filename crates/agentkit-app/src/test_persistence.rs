//! Explicit disk barrier for persisted-restart contracts, separate from turn completion.
use agentkit_client::{ClientOptions, Endpoint, RpcClient, RpcError};
use agentkit_protocol::generated as g;
/// v0.2.6 exportSession awaits flushSession before serializing its disk bundle.
/// An idle snapshot precedes that flush; terminating the process then is a crash test.
pub(crate) async fn durable_bundle(
    endpoint: Endpoint,
    workspace: &str,
    session: &str,
) -> Result<g::SessionBundle, RpcError> {
    let client = RpcClient::connect(ClientOptions::local(
        endpoint.url,
        endpoint.token,
        workspace.into(),
    ));
    let result = client.sessions().export(session).await;
    client.close();
    result
}
