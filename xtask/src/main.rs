//! Workspace development commands.
mod backend;
mod codegen;
mod fixtures;
mod i18n;
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
                "Usage: cargo xtask i18n [--check]\n       cargo xtask fixtures record <scenario|all>\n       cargo xtask fixtures replay <directory>\n       cargo xtask backend build [--contract]\n       cargo xtask codegen\n       cargo xtask codegen --check\nBuild the pinned headless backend without WebUI. Requires Bun >=1.3.14 and uv."
            );
            Ok(())
        }
        ["backend", "build"] => backend::build(false),
        ["backend", "build", "--contract"] => backend::build(true),
        ["fixtures", "record", scenario] => fixtures::record(scenario),
        ["fixtures", "replay", directory] => fixtures::replay_dir(std::path::Path::new(directory)),
        ["i18n"] => i18n::run(false),
        ["i18n", "--check"] => i18n::run(true),
        ["codegen"] => codegen::run(false),
        ["codegen", "--check"] => codegen::run(true),
        _ => anyhow::bail!("Unsupported command. Run cargo xtask --help."),
    }
}
