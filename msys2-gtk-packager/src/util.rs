use anyhow::ensure;
use anyhow::Context;
use std::collections::HashMap;
use std::ffi::OsString;
use std::process::Command;

/// A builder to build a cargo build command
pub struct CargoBuild {
    /// The `build` cargo subcommand command to run.
    ///
    /// This exists so that users can run things like `clippy`
    pub build: Option<String>,

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
            build: None,
            target: None,
            profile: None,
            bin: None,
            envs: HashMap::new(),
        }
    }

    /// Set the `build` command to use.
    pub fn build(&mut self, build: String) -> &mut Self {
        self.build = Some(build);
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
        let build = self.build.as_deref();
        let target = self.target.as_deref();
        let profile = self.profile.as_deref();
        let bin = self.bin.as_deref();
        let envs = &self.envs;

        let mut command = Command::new("cargo");
        command.arg(build.unwrap_or("build")).envs(envs);

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
