mod commands;
mod util;

use anyhow::Context as _;
use camino::Utf8PathBuf;

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

/// The CLI context
pub struct Context {
    /// The msys2 installation path
    pub msys2_installation_path: Utf8PathBuf,
}

impl Context {
    /// Make a new [`Context`].
    pub fn new() -> anyhow::Result<Self> {
        let msys2_installation_path = msys2_packager::util::locate_msys2_installation()
            .context("failed to locate MSYS2 installation")?;

        Ok(Self {
            msys2_installation_path,
        })
    }
}

fn main() -> anyhow::Result<()> {
    let options: Options = argh::from_env();
    let ctx = Context::new()?;

    match options.subcommand {
        Subcommand::Build(options) => {
            crate::commands::build::exec(options)?;
        }
        Subcommand::Run(options) => {
            crate::commands::run::exec(options)?;
        }
        Subcommand::Package(options) => {
            crate::commands::package::exec(ctx, options)?;
        }
    }

    Ok(())
}
