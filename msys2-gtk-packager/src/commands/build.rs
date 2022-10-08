use anyhow::ensure;
use anyhow::Context;
use std::path::PathBuf;

#[derive(Debug, argh::FromArgs)]
#[argh(
    subcommand,
    name = "build",
    description = "Build the GTK-rs application"
)]
pub struct Options {
    #[argh(option, description = "the target triple")]
    pub target: String,

    #[argh(
        option,
        description = "the build profile",
        default = "String::from(\"dev\")"
    )]
    pub profile: String,

    #[argh(option, long = "bin", description = "the binary name")]
    pub bin: String,

    #[argh(
        option,
        long = "build-subcommand",
        description = "the build subcommand to use in place of `build`, like `clippy`"
    )]
    pub build_subcommand: Option<String>,

    #[argh(
        option,
        long = "extra-library",
        description = "the name of an extra library to package"
    )]
    pub extra_libraries: Vec<String>,

    #[argh(
        option,
        short = 't',
        long = "theme",
        description = "the path to a theme to package"
    )]
    pub themes: Vec<PathBuf>,

    #[argh(
        option,
        long = "skip-package",
        default = "false",
        description = "skip the packaging step"
    )]
    pub skip_package: bool,

    #[argh(
        switch,
        description = "run the final binary. The advantage of this over specifiying a custom build command is that you can have the binary load custom themes"
    )]
    pub run: bool,
}

/// Exec the `build` subcommand.
pub fn exec(mut ctx: crate::Context, options: Options) -> anyhow::Result<()> {
    ctx.set_build_data(
        options.target.as_str(),
        options.profile.as_str(),
        options.bin.as_str(),
    )?;
    ctx.run_cargo_build(options.build_subcommand.as_deref())?;

    if !options.skip_package {
        ctx.package(false, &options.extra_libraries, &options.themes)?;
    }

    if options.run {
        let cmd = if options.skip_package {
            ctx.get_bin_path()?
        } else {
            ctx.get_packaged_bin_path()?
        };

        let status = std::process::Command::new(cmd)
            .status()
            .context("failed to run")?;

        ensure!(status.success());
    }

    Ok(())
}
