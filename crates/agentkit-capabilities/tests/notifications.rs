//! Explicit host-service check; never displays an unsolicited notification during ordinary tests.
#[test]
#[ignore = "Requires an explicit isolated native notification service"]
fn sends_private_approval_to_the_native_service()
-> Result<(), agentkit_capabilities::notifications::NotificationError> {
    agentkit_capabilities::notifications::approval(
        "Workbench",
        "dev.agentkit.workbench",
        "A conversation needs approval. Return to the workbench.",
    )
}
