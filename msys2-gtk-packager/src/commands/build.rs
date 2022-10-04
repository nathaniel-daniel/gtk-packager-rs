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
}

/// Exec the build subcommand
pub fn exec(options: Options) -> anyhow::Result<()> {
    crate::util::build(options.target.as_str(), options.profile.as_str(), false)?;

    Ok(())
}
