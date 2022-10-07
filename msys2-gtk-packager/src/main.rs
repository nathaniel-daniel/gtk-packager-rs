mod commands;
mod util;

use anyhow::Context as _;
use camino::Utf8PathBuf;
use msys2::Msys2Environment;

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

    /// The msys2 environment
    pub msys2_environment: Option<Msys2Environment>,

    /// The target triple
    pub target: Option<String>,
}

impl Context {
    /// Make a new [`Context`].
    pub fn new() -> anyhow::Result<Self> {
        let msys2_installation_path = msys2_packager::util::locate_msys2_installation()
            .context("failed to locate MSYS2 installation")?;

        Ok(Self {
            msys2_installation_path,
            msys2_environment: None,
            target: None,
        })
    }

    /// Set the target triple.
    ///
    /// This will update associated data, like the msys2 environment.
    pub fn set_target(&mut self, target: String) -> anyhow::Result<()> {
        let msys2_environment = msys2_packager::util::target_triple_to_msys2_environment(&target)
            .with_context(|| {
            format!("failed to translate `{}` into a MSYS2 environment", target)
        })?;

        self.target = Some(target);
        self.msys2_environment = Some(msys2_environment);
        Ok(())
    }

    /// Run a cargo build
    pub fn run_cargo_build(
        &self,
        profile: &str,
        bin: &str,
        build: Option<&str>,
    ) -> anyhow::Result<()> {
        let msys2_installation_path = &self.msys2_installation_path;
        let msys2_environment = self
            .msys2_environment
            .context("missing `msys2_environment`")?;
        let target = self.target.as_deref().context("missing `target`")?;

        let rel_prefix = msys2_environment.get_prefix().trim_start_matches('/');
        let env_sysroot = msys2_installation_path.join(rel_prefix);

        let mut cargo_build = crate::util::CargoBuild::new();
        if let Some(build) = build {
            cargo_build.build(build.into());
        }
        cargo_build
            .target(target.into())
            .profile(profile.into())
            .bin(bin.into())
            .env(
                "PATH".into(), // We use MSYS2's pkg-config
                std::env::join_paths(
                    std::iter::once(
                        msys2_installation_path
                            .join(&env_sysroot)
                            .join("bin")
                            .into(),
                    )
                    .chain(std::env::var_os("PATH").into_iter()),
                )?,
            )
            .env(
                "PKG_CONFIG_SYSROOT_DIR".into(),
                msys2_installation_path.into(),
            )
            .env(
                "PKG_CONFIG_LIBDIR".into(),
                msys2_installation_path
                    .join(env_sysroot)
                    .join("lib/pkgconfig")
                    .into(),
            )
            .exec()
    }
}

fn main() -> anyhow::Result<()> {
    let options: Options = argh::from_env();
    let ctx = Context::new()?;

    match options.subcommand {
        Subcommand::Build(options) => {
            crate::commands::build::exec(ctx, options)?;
        }
        Subcommand::Run(options) => {
            crate::commands::run::exec(ctx, options)?;
        }
        Subcommand::Package(options) => {
            crate::commands::package::exec(ctx, options)?;
        }
    }

    Ok(())
}
