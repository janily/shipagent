use agentkit_client::{RpcError, api::WorkbenchApi};
use agentkit_protocol::generated as g;
use agentkit_store::core::plan_dispatch as policy;
#[derive(Default)]
pub(super) enum Outcome {
    #[default]
    Unchanged,
    None,
    Pending {
        path: String,
        awaiting_compaction: bool,
    },
    Sent {
        path: String,
    },
}
fn failure() -> RpcError {
    RpcError::Wire(agentkit_protocol::WireError {
        code: agentkit_protocol::ErrorCode::HandlerError,
        message: "PLAN_NOT_CONFIRMED".into(),
        data: None,
    })
}
async fn execute_mode(api: &WorkbenchApi, id: &str, session: &g::Session) -> Result<(), RpcError> {
    if session.permission_mode == Some(g::PermissionMode::Safe) {
        api.set_session_mode(id, g::PermissionMode::AllowAll)
            .await?;
    }
    Ok(())
}
/// Session-bound work continues after the view closes once a host claim is accepted.
pub(super) async fn dispatch(
    api: &WorkbenchApi,
    id: &str,
    approved: &str,
    additional: &str,
) -> Result<Outcome, RpcError> {
    let session = api.session_generated(id).await?;
    if session.is_processing {
        return Ok(Outcome::default());
    }
    let Some(pending) = api.pending_plan(id).await? else {
        return Ok(Outcome::None);
    };
    let outcome = Outcome::Pending {
        path: pending.plan_path.clone(),
        awaiting_compaction: pending.awaiting_compaction,
    };
    if !policy::ready(&pending, false) || !api.claim_plan(id).await? {
        return Ok(outcome);
    }
    // No cancellation/retry after claim: a lost acknowledgement must never duplicate execution.
    execute_mode(api, id, &session).await?;
    let text = policy::approval_message(
        approved,
        additional,
        pending.draft_input_snapshot.as_deref().unwrap_or(""),
    );
    let ack = api
        .send(id, &text, &format!("plan-{}", uuid::Uuid::new_v4()))
        .await?;
    if !ack.accepted {
        return Err(failure());
    }
    api.clear_plan(id).await?;
    Ok(Outcome::Sent {
        path: pending.plan_path,
    })
}
pub(super) async fn accept(
    api: &WorkbenchApi,
    id: &str,
    message: &str,
    compact: bool,
    draft: &str,
    approved: &str,
    additional: &str,
) -> Result<Outcome, RpcError> {
    let session = api.session_generated(id).await?;
    let Some(plan) = session
        .messages
        .iter()
        .rev()
        .find(|m| m.role == g::MessageRole::Plan && m.hidden != Some(true))
    else {
        return Err(failure());
    };
    if session.is_processing || plan.id != message || policy::accepted(&session, message) {
        return Err(failure());
    }
    let path = policy::path(plan).unwrap_or("").to_string();
    if compact && path.is_empty() {
        return Err(failure());
    }
    if api.pending_plan(id).await?.is_some() {
        return Err(failure());
    }
    execute_mode(api, id, &session).await?;
    let text = if compact {
        api.prepare_plan(id, &path, draft).await?;
        "/compact".into()
    } else {
        policy::approval_message(approved, additional, draft)
    };
    let ack = api
        .send(id, &text, &format!("plan-{}", uuid::Uuid::new_v4()))
        .await?;
    if !ack.accepted {
        if compact {
            let _ = api.clear_plan(id).await;
        }
        return Err(failure());
    }
    Ok(if compact {
        Outcome::Pending {
            path,
            awaiting_compaction: true,
        }
    } else {
        Outcome::Sent { path }
    })
}
/// Stop at the first failure so another wakeup cannot disguise an uncertain send.
pub(super) async fn drain(
    api: &WorkbenchApi,
    id: &str,
    approved: &str,
    additional: &str,
    mut flight: policy::Flight,
) -> Result<Outcome, RpcError> {
    let mut latest = Outcome::Unchanged;
    loop {
        let outcome = dispatch(api, id, approved, additional).await?;
        if !matches!(outcome, Outcome::Unchanged) {
            latest = outcome;
        }
        if !flight.again() {
            return Ok(latest);
        }
    }
}
#[cfg(test)]
#[path = "plan_worker_tests.rs"]
mod tests;
