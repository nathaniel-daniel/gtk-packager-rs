use anyhow::ensure;
use anyhow::Context;
use std::process::Command;

/// Run a build
pub fn build(target: &str, profile: &str, bin: &str, run: bool) -> anyhow::Result<()> {
    let mut command = Command::new("cargo");
    command
        .arg(if run { "run" } else { "build" })
        .args(&["--target", target])
        .args(&["--profile", profile])
        .args(&["--bin", bin])
        .env("PKG_CONFIG_SYSROOT_DIR", "/"); // MSYS2's pkg-config does not support "cross" builds like the one we get by LITERALLY SPECIFYING ITSELF.

    let status = command.status().context("failed to run `cargo build`")?;

    ensure!(
        status.success(),
        "`{:?}` exited with nonzero exit code `{}`",
        command,
        status,
    );

    Ok(())
}
