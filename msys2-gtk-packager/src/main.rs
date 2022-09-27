mod commands;
mod packager;
mod util;

use crate::packager::FileFlags;
use crate::packager::Packager;
use crate::util::locate_msys2_installation;
use crate::util::msys2_to_windows;
use anyhow::bail;
use anyhow::ensure;
use anyhow::Context;
use std::fs::File;
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;
use walkdir::WalkDir;

#[derive(Debug, argh::FromArgs)]
#[argh(description = "A tool to aide in building GTK-rs programs for Windows, backed by MSYS2")]
struct Options {
    #[argh(subcommand)]
    subcommand: Subcommand,
}

#[derive(Debug, argh::FromArgs)]
#[argh(subcommand)]
enum Subcommand {
    Build(BuildOptions),
    Package(PackageOptions),
}

#[derive(Debug, argh::FromArgs)]
#[argh(
    subcommand,
    name = "build",
    description = "Build the GTK-rs application"
)]
struct BuildOptions {
    #[argh(option, description = "the target triple")]
    target: String,

    #[argh(
        option,
        description = "the build profile",
        default = "String::from(\"dev\")"
    )]
    profile: String,
}

#[derive(Debug, argh::FromArgs)]
#[argh(
    subcommand,
    name = "package",
    description = "Package the GTK-rs application"
)]
struct PackageOptions {
    #[argh(option, description = "the target triple")]
    target: String,

    #[argh(
        option,
        description = "do not attempt to build the project before packaging it",
        default = "false"
    )]
    no_build: bool,

    #[argh(
        option,
        description = "the build profile",
        default = "String::from(\"release\")"
    )]
    profile: String,

    #[argh(option, long = "bin", description = "the binary name")]
    bin: String,

    #[argh(
        option,
        long = "extra-library",
        description = "the name of an extra library to package"
    )]
    extra_libraries: Vec<String>,

    #[argh(
        option,
        short = 't',
        long = "theme",
        description = "the path to a theme to package"
    )]
    themes: Vec<String>,

    #[argh(switch, description = "whether to upx")]
    upx: bool,
}

fn build(target: &str, profile: &str) -> anyhow::Result<()> {
    let mut cargo_build_command = Command::new("cargo");
    cargo_build_command
        .arg("build")
        .args(&["--target", target])
        .args(&["--profile", profile])
        .env("PKG_CONFIG_SYSROOT_DIR", "/"); // MSYS2's pkg-config does not support "cross" builds like the one we get by LITERALLY SPECIFYING ITSELF.

    let cargo_build_status = cargo_build_command
        .status()
        .context("failed to run `cargo build`")?;

    ensure!(
        cargo_build_status.success(),
        "cargo build exited with a nonzero exit code"
    );

    Ok(())
}

fn main() -> anyhow::Result<()> {
    let options: Options = argh::from_env();
    match options.subcommand {
        Subcommand::Build(options) => {
            build(options.target.as_str(), options.profile.as_str())?;
        }
        Subcommand::Package(options) => {
            if !options.no_build {
                build(options.target.as_str(), options.profile.as_str())?;
            }

            let metadata = cargo_metadata::MetadataCommand::new()
                .exec()
                .context("failed to get cargo metadata")?;

            let msys2_installation_path =
                locate_msys2_installation().context("failed to locate MSYS2 installation")?;

            // Validate `options.bin`
            let bin_is_valid = metadata
                .packages
                .iter()
                .flat_map(|package| {
                    package
                        .targets
                        .iter()
                        .filter(|target| target.kind.iter().any(|kind| kind == "bin"))
                })
                .any(|target| target.name == options.bin);
            ensure!(bin_is_valid, "`{}` is not a valid bin", options.bin);
            let bin_name = format!("{}.exe", options.bin);

            let profile = options.profile;
            let profile_dir = metadata.target_directory.join(options.target.as_str());
            let bin_dir = profile_dir.join(profile);

            // TODO: Allow user to customize name/autogenerate?
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
            let mut packager = Packager::new(package_dir.clone().into());
            packager
                .resolve_unknown_libraries(true)
                .upx(options.upx)
                .add_file(
                    Some(src_bin_path.clone().into()),
                    bin_name.into(),
                    FileFlags::EXE | FileFlags::UPX | FileFlags::ADD_DEPS,
                )
                .add_file(
                    None,
                    "gdbus.exe".into(), // gdbus.exe is needed for GTK apps to function on Windows
                    FileFlags::EXE | FileFlags::UPX | FileFlags::ADD_DEPS,
                );

            // Copy extra libraries
            if !options.extra_libraries.is_empty() {
                for library in options.extra_libraries.iter() {
                    packager.add_file(
                        None,
                        library.into(),
                        FileFlags::LIB | FileFlags::UPX | FileFlags::ADD_DEPS,
                    );
                }
            }

            let gtk_lib_dir = {
                let output = Command::new("pkg-config")
                    .arg("gtk4")
                    .arg("--libs-only-L")
                    .output()
                    .context("failed to run pkg-config")?;
                ensure!(
                    output.status.success(),
                    "failed to locate `gtk4` via pkg-config"
                );
                let stdout = std::str::from_utf8(&output.stdout)
                    .context("pkg-config output is not utf8")?
                    .trim()
                    .trim_start_matches("-L");

                PathBuf::from(stdout)
            };

            let gstreamer_lib_dir = {
                // Locate gstreamer lib dir
                let output = Command::new("pkg-config")
                    .arg("gstreamer-1.0")
                    .arg("--libs-only-L")
                    .output()
                    .context("failed to run pkg-config")?;
                ensure!(
                    output.status.success(),
                    "failed to locate `gstreamer-1.0` via pkg-config"
                );
                let stdout = std::str::from_utf8(&output.stdout)
                    .context("pkg-config output is not utf8")?
                    .trim()
                    .trim_start_matches("-L");

                PathBuf::from(stdout)
            };

            let gstreamer_plugins_dir = {
                // Locate gstreamer lib dir
                let output = Command::new("pkg-config")
                    .arg("gstreamer-plugins-base-1.0")
                    .arg("--libs-only-L")
                    .output()
                    .context("failed to run pkg-config")?;
                ensure!(
                    output.status.success(),
                    "failed to locate `gstreamer-plugins-base-1.0` via pkg-config"
                );
                let stdout = std::str::from_utf8(&output.stdout)
                    .context("pkg-config output is not utf8")?
                    .trim()
                    .trim_start_matches("-L");

                PathBuf::from(stdout)
            };

            packager
                .add_file(
                    Some(gtk_lib_dir.join("gtk-4.0/4.0.0/media/libmedia-gstreamer.dll")), // TODO: Allow customization based on gtk target
                    "lib/gtk-4.0/4.0.0/media/libmedia-gstreamer.dll".into(),
                    FileFlags::LIB | FileFlags::UPX | FileFlags::ADD_DEPS,
                )
                .add_file(
                    Some(gstreamer_lib_dir.join("gstreamer-1.0/libgstcoreelements.dll")),
                    "lib/gstreamer-1.0/libgstcoreelements.dll".into(),
                    FileFlags::LIB | FileFlags::UPX | FileFlags::ADD_DEPS,
                );

            // libgstplayback.dll
            {
                // Locate gstreamer lib dir
                let output = Command::new("pkg-config")
                    .arg("gstreamer-plugins-base-1.0")
                    .arg("--libs-only-L")
                    .output()
                    .context("failed to run pkg-config")?;
                ensure!(
                    output.status.success(),
                    "failed to locate `gstreamer-plugins-base-1.0` via pkg-config"
                );
                let stdout = std::str::from_utf8(&output.stdout)
                    .context("pkg-config output is not utf8")?
                    .trim()
                    .trim_start_matches("-L");

                let src = PathBuf::from(msys2_to_windows(
                    Path::new(stdout).join("gstreamer-1.0/libgstplayback.dll"),
                )?);

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
                        Some(gstreamer_plugins_dir.join("gstreamer-1.0").join(plugin)),
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

                let mut file = File::create(&gtk.join("settings.ini"))
                    .context("failed to open settings.ini")?;
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
                            std::fs::copy(dir_entry.path(), dest_path)
                                .context("failed to copy file")?;
                        } else if file_type.is_dir() {
                            std::fs::create_dir(&dest_path).context("failed to create dir")?;
                        } else {
                            bail!("symlink in theme folder");
                        }
                    }
                }
            }
        }
    }

    Ok(())
}
