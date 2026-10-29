//! Workspace development commands.
mod backend;
fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>()
        .as_slice()
    {
        [] | ["--help"] | ["-h"] => {
            println!(
                "Usage: cargo xtask backend build\nBuild the pinned headless backend without WebUI. Requires Bun >=1.3.14 and uv."
            );
            Ok(())
        }
        ["backend", "build"] => backend::build(),
        _ => anyhow::bail!("Unsupported command. Run cargo xtask --help."),
    }
}
