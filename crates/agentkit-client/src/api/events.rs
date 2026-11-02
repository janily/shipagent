//! Typed argument decoding; errors reach subscribers instead of disappearing.
use super::*;
use agentkit_protocol::{DecodedSessionEvent, constants::channels as c};
pub struct Events<'a>(pub(super) &'a RpcClient);
pub const V1_EVENT_CHANNELS: &[&str] = &[
    c::sessions::EVENT,
    c::sessions::UNREAD_SUMMARY_CHANGED,
    c::sessions::FILES_CHANGED,
    c::skills::CHANGED,
    c::llm_connections::CHANGED,
    c::permissions::DEFAULTS_CHANGED,
    c::server::STATUS_CHANGED,
    c::server::SHUTTING_DOWN,
];
impl RpcClient {
    /// Typed v1 push-channel subscriptions. Drop receivers to unsubscribe.
    pub fn events(&self) -> Events<'_> {
        Events(self)
    }
}
impl Events<'_> {
    fn one<T: DeserializeOwned + Send + 'static>(
        &self,
        channel: &str,
    ) -> async_channel::Receiver<Result<T, RpcError>> {
        self.0.mapped_subscription(channel, |args| {
            let (value,): (T,) = serde_json::from_value(Value::Array(args))?;
            Ok(value)
        })
    }
    pub fn session(&self) -> async_channel::Receiver<Result<DecodedSessionEvent, RpcError>> {
        self.one(c::sessions::EVENT)
    }
    pub fn unread_summary(&self) -> async_channel::Receiver<Result<g::UnreadSummary, RpcError>> {
        self.one(c::sessions::UNREAD_SUMMARY_CHANGED)
    }
    pub fn files_changed(&self) -> async_channel::Receiver<Result<String, RpcError>> {
        self.one(c::sessions::FILES_CHANGED)
    }
    pub fn skills_changed(
        &self,
    ) -> async_channel::Receiver<Result<(String, Vec<g::LoadedSkill>), RpcError>> {
        self.0.mapped_subscription(c::skills::CHANGED, |args| {
            Ok(serde_json::from_value(Value::Array(args))?)
        })
    }
    pub fn connections_changed(&self) -> async_channel::Receiver<Result<(), RpcError>> {
        self.0
            .mapped_subscription(c::llm_connections::CHANGED, |args| {
                Ok(serde_json::from_value(if args.is_empty() {
                    Value::Null
                } else {
                    Value::Array(args)
                })?)
            })
    }
    pub fn permissions_changed(&self) -> async_channel::Receiver<Result<(), RpcError>> {
        self.one(c::permissions::DEFAULTS_CHANGED)
    }
    fn notice(&self, channel: &str) -> async_channel::Receiver<Result<ServerNotice, RpcError>> {
        self.0.mapped_subscription(channel, |args| {
            Ok(ServerNotice {
                arguments: serde_json::from_value(Value::Array(args))?,
            })
        })
    }
    pub fn server_status(&self) -> async_channel::Receiver<Result<ServerNotice, RpcError>> {
        self.notice(c::server::STATUS_CHANGED)
    }
    pub fn server_shutting_down(&self) -> async_channel::Receiver<Result<ServerNotice, RpcError>> {
        self.notice(c::server::SHUTTING_DOWN)
    }
}
