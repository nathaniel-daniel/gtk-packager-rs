use anyhow::bail;
use anyhow::Context;
use msys2_packager::packager::FileFlags;
use msys2_packager::packager::Packager;
use std::fs::File;
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;
use walkdir::WalkDir;

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
    pub themes: Vec<String>,

    #[argh(switch, description = "whether to upx")]
    pub upx: bool,
}

/// Run the `package` subcommand.
pub fn exec(mut ctx: crate::Context, options: Options) -> anyhow::Result<()> {
    ctx.set_target(options.target.clone())?;
    ctx.set_bin(options.bin.clone())?;
    ctx.set_profile(options.profile.clone())?;

    if !options.no_build {
        ctx.run_cargo_build(None)?;
    }

    let profile = options.profile;
    let profile_dir = ctx
        .cargo_metadata
        .target_directory
        .join(options.target.as_str());
    let bin_dir = profile_dir.join(profile);
    let bin_name = format!("{}.exe", options.bin);

    // TODO: autogenerate
    let package_dir = profile_dir.join(env!("CARGO_CRATE_NAME")).join(options.bin);
    match std::fs::remove_dir_all(&package_dir) {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            // Pass, it already does not exist.
        }
        Err(e) => {
            return Err(e).context("failed to remove old package dir");
        }
    }

    let src_bin_path = bin_dir.join(&bin_name);
    let mut packager = Packager::new(
        ctx.msys2_installation_path,
        ctx.msys2_environment.context("missing msys2 environment")?,
        package_dir.clone().into(),
    );
    packager
        .resolve_unknown_libraries(true)
        .upx(options.upx)
        .add_file(
            Some(src_bin_path.into()),
            bin_name.into(),
            FileFlags::EXE | FileFlags::UPX | FileFlags::ADD_DEPS,
        )
        .add_file(
            None,
            "gdbus.exe".into(), // gdbus.exe is needed for GTK apps to function on Windows
            FileFlags::EXE | FileFlags::UPX | FileFlags::ADD_DEPS,
        );

    // Copy extra libraries
    for library in options.extra_libraries.iter() {
        packager.add_file(
            None,
            library.into(),
            FileFlags::LIB | FileFlags::UPX | FileFlags::ADD_DEPS,
        );
    }

    let msys2_environment_path = packager.get_msys2_environment_path();
    packager.add_file(
        Some(msys2_environment_path.join_os("lib/gtk-4.0/4.0.0/media/libmedia-gstreamer.dll")), // TODO: Allow customization based on gtk target and media backend
        "lib/gtk-4.0/4.0.0/media/libmedia-gstreamer.dll".into(),
        FileFlags::LIB | FileFlags::UPX | FileFlags::ADD_DEPS,
    );

    packager.add_file(
        Some(msys2_environment_path.join_os("lib/gstreamer-1.0/libgstcoreelements.dll")),
        "lib/gstreamer-1.0/libgstcoreelements.dll".into(),
        FileFlags::LIB | FileFlags::UPX | FileFlags::ADD_DEPS,
    );

    // libgstplayback.dll
    {
        let src = msys2_environment_path.join_os("lib/gstreamer-1.0/libgstplayback.dll");
        packager.add_file(
            Some(src),
            "lib/gstreamer-1.0/libgstplayback.dll".into(),
            FileFlags::LIB | FileFlags::UPX | FileFlags::ADD_DEPS,
        );

        // Extra GST plugins
        let extra_plugins = [
            // Mandatory, as gstreamer as part of gtk should have webm support with vp8 and vp9 codecs
            // This allows important elements to work, like the video element.
            "libgsttypefindfunctions.dll",
            "libgstvpx.dll",
            "libgstmatroska.dll",
            "libgstvideoconvert.dll",
            "libgstaudioconvert.dll",
            "libgstaudiofx.dll",
            "libgstvideoscale.dll",
            "libgstvideofilter.dll",
            "libgstdeinterlace.dll",
            "libgstvolume.dll",
            "libgstaudioresample.dll",
            "libgstwasapi.dll",
            // MP4/H264 support
            "libgstisomp4.dll",
            "libgstvideoparsersbad.dll",
            "libgstmediafoundation.dll",
            "libgstopenh264.dll",
            // AAC
            "libgstaudioparsers.dll",
            "libgstfaad.dll",
            "libgstmpg123.dll",
            // Opus
            "libgstopus.dll",
            "libgstautodetect.dll",
            // Nvidia acceleration
            "libgstnvcodec.dll",
            // FFMPeg
            // "libgstlibav.dll",
            "libgstcodecalpha.dll",
        ];

        // If video elements are used, you need a basic gstreamer setup
        let gstreamer = Path::new("lib/gstreamer-1.0");

        for plugin in extra_plugins {
            let dest = gstreamer.join(plugin);
            packager.add_file(
                Some(
                    msys2_environment_path
                        .join("lib")
                        .join_os("gstreamer-1.0")
                        .join(plugin),
                ),
                dest,
                FileFlags::UPX | FileFlags::LIB | FileFlags::ADD_DEPS,
            );
        }
    }

    packager.package().context("failed to package")?;

    // Write out settings.ini
    {
        let etc = package_dir.join("etc");
        std::fs::create_dir(&etc).context("failed to create etc dir")?;

        // TODO: Allow customization based on gtk target
        let gtk = etc.join("gtk-4.0");
        std::fs::create_dir(&gtk).context("failed to create gtk dir")?;

        let mut file =
            File::create(&gtk.join("settings.ini")).context("failed to open settings.ini")?;
        // TODO: Allow customization
        file.write_all(b"[Settings]\ngtk-theme-name=Dracula\n")
            .context("failed to write out settings.ini")?;
        file.flush().context("failed to flush")?;
        file.sync_all().context("failed to sync")?;
    }

    // Copy themes
    if !options.themes.is_empty() {
        let share = package_dir.join("share");
        std::fs::create_dir(&share).context("failed to create share dir")?;

        let themes = share.join("themes");
        std::fs::create_dir(&themes).context("failed to create themes dir")?;

        for theme in options.themes {
            let theme = PathBuf::from(theme)
                .canonicalize()
                .context("failed to canonicalize theme path")?;
            let theme_name = theme.file_name().context("theme has no name")?;
            let theme_dest = themes.join_os(theme_name);
            for dir_entry in WalkDir::new(&theme) {
                let dir_entry = dir_entry.context("failed to get dir entry")?;
                let relative_path = dir_entry
                    .path()
                    .strip_prefix(&theme)
                    .context("dir entry path is not prefixed by the theme dir")?;

                let dest_path = theme_dest.join(relative_path);
                let file_type = dir_entry.file_type();
                if file_type.is_file() {
                    std::fs::copy(dir_entry.path(), dest_path).context("failed to copy file")?;
                } else if file_type.is_dir() {
                    std::fs::create_dir(&dest_path).context("failed to create dir")?;
                } else {
                    bail!("symlink in theme folder");
                }
            }
        }
    }

    Ok(())
}
