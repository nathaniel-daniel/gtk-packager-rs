use anyhow::ensure;
use anyhow::Context;
use std::collections::HashMap;
use std::ffi::OsString;
use std::process::Command;

/// Run a build
pub fn build(target: &str, profile: &str, bin: &str, run: bool) -> anyhow::Result<()> {
    let mut command = CargoBuild::new()
        .run(run)
        .target(target.into())
        .profile(profile.into())
        .bin(bin.into())
        .env("PKG_CONFIG_SYSROOT_DIR".into(), "/".into()) // MSYS2's pkg-config does not support "cross" builds like the one we get by LITERALLY SPECIFYING ITSELF.
        .build_command()?;

    let status = command.status().context("failed to run `cargo build`")?;

    ensure!(
        status.success(),
        "`{:?}` exited with nonzero exit code `{}`",
        command,
        status,
    );

    Ok(())
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
}
