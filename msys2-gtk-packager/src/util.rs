use anyhow::ensure;
use anyhow::Context;
use camino::Utf8Path;
use msys2::Msys2Environment;
use std::collections::HashMap;
use std::ffi::OsString;
use std::process::Command;

/// Run a build
pub fn build(
    target: &str,
    profile: &str,
    bin: &str,
    msys2_installation_path: &Utf8Path,
    msys2_environment: Msys2Environment,
    run: bool,
) -> anyhow::Result<()> {
    let env_sysroot =
        msys2_installation_path.join(msys2_environment.get_prefix().trim_start_matches('/'));

    CargoBuild::new()
        .run(run)
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

/// A builder to build a cargo build command
pub struct CargoBuild {
    /// If `true`, `cargo run` should be used instead of `cargo build`.
    pub run: bool,

    /// The target triple
    pub target: Option<String>,

    /// The build profile
    pub profile: Option<String>,

    /// The bin to build
    pub bin: Option<String>,

    /// The environment for the command
    pub envs: HashMap<OsString, OsString>,
}

impl CargoBuild {
    /// Make a new [`CargoBuild`].
    pub fn new() -> Self {
        Self {
            run: false,
            target: None,
            profile: None,
            bin: None,
            envs: HashMap::new(),
        }
    }

    /// Set whether `cargo run` should be used.
    pub fn run(&mut self, run: bool) -> &mut Self {
        self.run = run;
        self
    }

    /// Set the target.
    pub fn target(&mut self, target: String) -> &mut Self {
        self.target = Some(target);
        self
    }

    /// Set the profile.
    pub fn profile(&mut self, profile: String) -> &mut Self {
        self.profile = Some(profile);
        self
    }

    /// Set the bin to build
    pub fn bin(&mut self, bin: String) -> &mut Self {
        self.bin = Some(bin);
        self
    }

    /// Add an env
    pub fn env(&mut self, key: OsString, value: OsString) -> &mut Self {
        self.envs.insert(key, value);
        self
    }

    /// Build this command.
    pub fn build_command(&self) -> anyhow::Result<Command> {
        let run = self.run;
        let target = self.target.as_deref();
        let profile = self.profile.as_deref();
        let bin = self.bin.as_deref();
        let envs = &self.envs;

        let mut command = Command::new("cargo");
        command.arg(if run { "run" } else { "build" }).envs(envs);

        if let Some(target) = target {
            command.args(&["--target", target]);
        }

        if let Some(profile) = profile {
            command.args(&["--profile", profile]);
        }

        if let Some(bin) = bin {
            command.args(&["--bin", bin]);
        }

        Ok(command)
    }

    /// Run this command.
    pub fn exec(&self) -> anyhow::Result<()> {
        let mut command = self.build_command()?;
        let status = command
            .status()
            .with_context(|| format!("failed to run `{:?}`", command))?;

        ensure!(
            status.success(),
            "`{:?}` exited with nonzero exit code `{}`",
            command,
            status,
        );

        Ok(())
    }
}
