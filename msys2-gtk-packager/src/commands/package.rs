use std::path::PathBuf;

#[derive(Debug, argh::FromArgs)]
#[argh(
    subcommand,
    name = "package",
    description = "Package the GTK-rs application"
)]
pub struct Options {
    #[argh(option, description = "the target triple")]
    pub target: String,

    #[argh(
        option,
        description = "do not attempt to build the project before packaging it",
        default = "false"
    )]
    pub no_build: bool,

    #[argh(
        option,
        description = "the build profile",
        default = "String::from(\"release\")"
    )]
    pub profile: String,

    #[argh(option, long = "bin", description = "the binary name")]
    pub bin: String,

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

    #[argh(switch, description = "whether to upx")]
    pub upx: bool,
}

/// Run the `package` subcommand.
pub fn exec(mut ctx: crate::Context, options: Options) -> anyhow::Result<()> {
    ctx.set_build_data(
        options.target.as_str(),
        options.profile.as_str(),
        options.bin.as_str(),
    )?;

    if !options.no_build {
        ctx.run_cargo_build(None)?;
    }

    ctx.package(options.upx, &options.extra_libraries, &options.themes)?;

    Ok(())
}
