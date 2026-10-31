//! Workspace development commands.
mod backend;
mod codegen;
mod package;
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
                "Usage: cargo xtask backend build\n       cargo xtask codegen\n       cargo xtask codegen --check\n       cargo xtask package --target macos-arm64 --sign ad-hoc [--probe]\nBuild the pinned headless backend without WebUI. Requires Bun >=1.3.14 and uv."
            );
            Ok(())
        }
        ["package", rest @ ..] => {
            package::run(&rest.iter().map(|v| v.to_string()).collect::<Vec<_>>())
        }
        ["backend", "build"] => backend::build(),
        ["codegen"] => codegen::run(false),
        ["codegen", "--check"] => codegen::run(true),
        _ => anyhow::bail!("Unsupported command. Run cargo xtask --help."),
    }
}
