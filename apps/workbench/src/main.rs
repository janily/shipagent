//! AgentKit native reference application.
fn main() -> anyhow::Result<()> {
    agentkit_app::run(include_str!("../brand.toml"))
}
