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
}

/// Exec the `build` subcommand.
pub fn exec(mut ctx: crate::Context, options: Options) -> anyhow::Result<()> {
    ctx.set_target(options.target.clone())?;
    ctx.set_bin(options.bin.clone())?;
    ctx.set_profile(options.profile.clone())?;

    ctx.run_cargo_build(options.build_subcommand.as_deref())?;
    Ok(())
}
