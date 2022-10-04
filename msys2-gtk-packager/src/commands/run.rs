#[derive(Debug, argh::FromArgs)]
#[argh(subcommand, name = "run", description = "Run the GTK-rs application")]
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
}

/// Run the `run` subcommand
pub fn exec(options: Options) -> anyhow::Result<()> {
    crate::util::build(
        options.target.as_str(),
        options.profile.as_str(),
        options.bin.as_str(),
        true,
    )?;
    Ok(())
}
