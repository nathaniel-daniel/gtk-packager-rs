mod commands;
mod util;

#[derive(Debug, argh::FromArgs)]
#[argh(description = "A tool to aide in building GTK-rs programs for Windows, backed by MSYS2")]
struct Options {
    #[argh(subcommand)]
    subcommand: Subcommand,
}

#[derive(Debug, argh::FromArgs)]
#[argh(subcommand)]
enum Subcommand {
    Build(crate::commands::build::Options),
    Run(crate::commands::run::Options),
    Package(crate::commands::package::Options),
}

fn main() -> anyhow::Result<()> {
    let options: Options = argh::from_env();
    match options.subcommand {
        Subcommand::Build(options) => {
            crate::commands::build::exec(options)?;
        }
        Subcommand::Run(options) => {
            crate::commands::run::exec(options)?;
        }
        Subcommand::Package(options) => {
            crate::commands::package::exec(options)?;
        }
    }

    Ok(())
}
