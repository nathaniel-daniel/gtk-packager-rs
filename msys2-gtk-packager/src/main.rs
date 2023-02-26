mod commands;
mod util;

use anyhow::bail;
use anyhow::ensure;
use anyhow::Context as _;
use camino::Utf8Path;
use camino::Utf8PathBuf;
use msys2::Msys2Environment;
use msys2_packager::packager::FileFlags;
use msys2_packager::packager::Packager;
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;
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
    Build(crate::commands::build::Options),
    Package(crate::commands::package::Options),
}

/// The CLI context
pub struct Context {
    /// The msys2 installation path
    pub msys2_installation_path: Utf8PathBuf,

    /// The msys2 environment
    pub msys2_environment: Option<Msys2Environment>,

    /// Cargo metadata
    pub cargo_metadata: cargo_metadata::Metadata,

    /// The `profile` to build
    pub profile: Option<String>,

    /// Data needed to perform a `cargo build`.
    pub build_data: Option<BuildData>,
}

impl Context {
    /// Make a new [`Context`].
    pub fn new() -> anyhow::Result<Self> {
        let msys2_installation_path = msys2_packager::util::locate_msys2_installation()
            .context("failed to locate MSYS2 installation")?;

        // This is required, as all current subcommands will need this data.
        //
        // If this changes in the future, make this optional.
        let cargo_metadata = cargo_metadata::MetadataCommand::new()
            .exec()
            .context("failed to get cargo metadata")?;

        Ok(Self {
            msys2_installation_path,
            msys2_environment: None,
            cargo_metadata,
            profile: None,
            build_data: None,
        })
    }

    /// Run a cargo build.
    ///
    /// `bin` is not validated before the command is invoked.
    pub fn run_cargo_build(&self, build: Option<&str>) -> anyhow::Result<()> {
        let msys2_installation_path = &self.msys2_installation_path;
        let msys2_environment = self
            .msys2_environment
            .context("missing `msys2_environment`")?;
        let build_data = self.build_data.as_ref().context("missing build data")?;
        let target = build_data.target.as_str();
        let profile = build_data.profile.as_str();
        let bin = build_data.bin.as_str();

        let rel_prefix = msys2_environment.get_prefix().trim_start_matches('/');
        let env_sysroot = msys2_installation_path.join(rel_prefix);

        let mut cargo_build = crate::util::CargoBuild::new();
        if let Some(build) = build {
            cargo_build.build(build.into());
        }
        cargo_build
            .target(target.into())
            .profile(profile.into())
            .bin(bin.into())
            .env(
                // TODO: Consider ripping out pkg-config and locating all these libs manually so users can use pkg-config for other stuff, or extend those env vars.
                "PKG_CONFIG_SYSROOT_DIR".into(),
                msys2_installation_path.into(),
            )
            .env(
                "PKG_CONFIG_LIBDIR".into(),
                msys2_installation_path
                    .join(env_sysroot)
                    .join("lib/pkgconfig")
                    .into(),
            )
            .exec()
    }

    /// Validate and set cargo build data.
    ///
    /// This will update associated data, like the msys2 environment.
    ///
    /// Currently, `profile` is not validated.
    pub fn set_build_data(&mut self, target: &str, profile: &str, bin: &str) -> anyhow::Result<()> {
        let msys2_environment = msys2_packager::util::target_triple_to_msys2_environment(target)
            .with_context(|| {
                format!("failed to translate `{target}` into a MSYS2 environment")
            })?;

        // Validate bin
        let bin_is_valid = self
            .cargo_metadata
            .packages
            .iter()
            .flat_map(|package| {
                package
                    .targets
                    .iter()
                    .filter(|target| target.kind.iter().any(|kind| kind == "bin"))
            })
            .any(|target| target.name == bin);

        ensure!(bin_is_valid, "`{}` is not a valid bin", bin);

        self.msys2_environment = Some(msys2_environment);
        self.build_data = Some(BuildData {
            target: target.into(),
            profile: profile.into(),
            bin: bin.into(),
        });
        Ok(())
    }

    /// Get the path to the binary that cargo will produce
    pub fn get_bin_path(&self) -> anyhow::Result<Utf8PathBuf> {
        let build_data = self.build_data.as_ref().context("missing build data")?;

        let mut profile = build_data.profile.as_str();
        // "dev" profile maps to "debug" in target folder
        if profile == "dev" {
            profile = "debug";
        }

        let mut path = self.cargo_metadata.target_directory.clone();
        path.extend([
            build_data.target.as_str(),
            profile,
            build_data.get_bin_name().as_str(),
        ]);
        Ok(path)
    }

    /// Get the path where the packager will output the binary
    pub fn get_packaged_bin_path(&self) -> anyhow::Result<Utf8PathBuf> {
        let build_data = self.build_data.as_ref().context("missing build data")?;
        Ok(self
            .get_package_out_dir()?
            .join(build_data.get_bin_name().as_str()))
    }

    /// Get the out dir where package artifacts will be placed.
    pub fn get_package_out_dir(&self) -> anyhow::Result<Utf8PathBuf> {
        let build_data = self.build_data.as_ref().context("missing build data")?;
        let target = build_data.target.as_str();
        let mut profile = build_data.profile.as_str();
        let bin = build_data.bin.as_str();

        // "dev" profile maps to "debug" in target folder
        if profile == "dev" {
            profile = "debug";
        }

        let target_dir = &self.cargo_metadata.target_directory.join(target);

        // This is the dir where we can place whatever we want in.
        //
        // We will mimic cargo's structure of {target}/{profile}/{bin}
        let base_dir = target_dir.join(env!("CARGO_CRATE_NAME"));

        let out_dir = base_dir.join(target).join(profile).join(bin);

        std::fs::create_dir_all(&out_dir).context("failed to create packaging dir")?;

        Ok(out_dir)
    }

    /// Package a binary.
    ///
    /// Note that this will not perform a build before-hand.
    pub fn package(
        &self,
        upx: bool,
        extra_libraries: &[String],
        themes: &[PathBuf],
    ) -> anyhow::Result<Packager> {
        let msys2_environment = self
            .msys2_environment
            .context("missing msys2 environment")?;
        let build_data = self.build_data.as_ref().context("missing build data")?;
        let package_dir = self.get_package_out_dir()?;

        // TODO: Consider making this step optional.
        // With dependency tracking, we could greatly speed up builds by not copying dependant artifacts.
        // Clear out old contents
        match std::fs::remove_dir_all(&package_dir) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                // Pass, it already does not exist.
            }
            Err(e) => {
                return Err(e).context("failed to remove old package dir");
            }
        }

        let mut packager = Packager::new(
            self.msys2_installation_path.clone(),
            msys2_environment,
            package_dir.clone().into(),
        );
        packager
            .resolve_unknown_libraries(true)
            .upx(upx)
            .add_file(
                Some(self.get_bin_path()?.into()),
                build_data.get_bin_name().into(),
                FileFlags::EXE | FileFlags::UPX | FileFlags::ADD_DEPS,
            )
            .add_file(
                None,
                "gdbus.exe".into(), // gdbus.exe is needed for GTK apps to function on Windows
                FileFlags::EXE | FileFlags::UPX | FileFlags::ADD_DEPS,
            );

        // TODO: This should be fleshed-out more as a generic file-copying option.
        // Copy extra libraries
        for library in extra_libraries.iter() {
            packager.add_file(
                None,
                library.into(),
                FileFlags::LIB | FileFlags::UPX | FileFlags::ADD_DEPS,
            );
        }

        // Add files needed for the media backend (I think only for GTK4).
        // TODO: This is technically optional, maybe allow users to disable to inclusion of the media backend?
        // TODO: Allow customization based on gtk target and media backend
        let msys2_environment_path = packager.get_msys2_environment_path();
        packager.add_file(
            Some(msys2_environment_path.join_os("lib/gtk-4.0/4.0.0/media/libmedia-gstreamer.dll")),
            "lib/gtk-4.0/4.0.0/media/libmedia-gstreamer.dll".into(),
            FileFlags::LIB | FileFlags::UPX | FileFlags::ADD_DEPS,
        );
        // DLLS included as part of gstreamer:
        let gstreamer_dlls = &[
            "libgstbase-1.0-0.dll",
            // "libgstcheck-1.0-0.dll", // Doesn't always seem needed?
            // "libgstcontroller-1.0-0.dll", // Doesn't always seem needed?
            // "libgstnet-1.0-0.dll", // Doesn't always seem needed?
            "libgstreamer-1.0-0.dll",
        ];
        for dll in gstreamer_dlls.iter() {
            packager.add_file(
                None,
                dll.into(),
                FileFlags::LIB | FileFlags::UPX | FileFlags::ADD_DEPS,
            );
        }

        let gstreamer_plugins = [
            // These elements are needed for a minimal gstreamer install that can play videos:
            "libgstcoreelements.dll",
            "libgstplayback.dll",
            "libgstvideoconvert.dll",
            "libgstaudioconvert.dll",
            "libgstvolume.dll",
            "libgstaudioresample.dll",
            "libgstaudiofx.dll",
            "libgstvideoscale.dll",
            "libgstvideofilter.dll",
            "libgstdeinterlace.dll",
            "libgsttypefindfunctions.dll",
            "libgstautodetect.dll",
            "libgstcodecalpha.dll",
            // These elements are needed for webms with vp8/9 codecs, which are suggested to be supported by GTK4 distributions:
            // TODO: Allow users to disable support.
            "libgstvpx.dll",
            "libgstmatroska.dll",
            // This is needed for audio playback on windows:
            "libgstwasapi.dll",
            // Opus Support:
            "libgstopus.dll",
            // MP4/H264 support:
            // TODO: Allow users to disable
            "libgstisomp4.dll",
            "libgstvideoparsersbad.dll",
            "libgstopenh264.dll",
            // Windows media foundation acceleration:
            // TODO: Allow users to disable
            "libgstmediafoundation.dll",
            // AAC Support:
            // TODOL Allow users to disable
            "libgstaudioparsers.dll",
            "libgstfaad.dll",
            "libgstmpg123.dll",
            // Nvidia acceleration
            // TODO: Allow users to disable
            "libgstnvcodec.dll",
            // FFMPeg
            // "libgstlibav.dll", // Really bloated, but by far the best video playing support plugin
        ];

        for plugin in &gstreamer_plugins {
            // I'm fairly certain only gstreamer-1.0 is supported with gtk4,
            // so this probably needs no config options.
            packager.add_file(
                Some(msys2_environment_path.join_os(format!("lib/gstreamer-1.0/{plugin}"))),
                format!("lib/gstreamer-1.0/{plugin}").into(),
                FileFlags::LIB | FileFlags::UPX | FileFlags::ADD_DEPS,
            );
        }

        // Copy themes
        if !themes.is_empty() {
            let themes_dest = Utf8Path::new("share").join("themes");

            for theme in themes {
                let theme = PathBuf::from(theme)
                    .canonicalize()
                    .context("failed to canonicalize theme path")?;
                let theme_name = theme.file_name().context("theme has no name")?;
                let theme_dest = themes_dest.join_os(theme_name);
                for dir_entry in WalkDir::new(&theme) {
                    let dir_entry = dir_entry.context("failed to get dir entry")?;
                    let relative_path = dir_entry
                        .path()
                        .strip_prefix(&theme)
                        .context("dir entry path is not prefixed by the theme dir")?;

                    let dest_path = theme_dest.join(relative_path);
                    let file_type = dir_entry.file_type();
                    if file_type.is_file() {
                        packager.add_file(
                            Some(dir_entry.path().into()),
                            dest_path,
                            FileFlags::empty(),
                        );
                    } else if file_type.is_dir() {
                        // Pass, packager will make it for us
                    } else {
                        bail!("symlink in theme folder");
                    }
                }
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
                File::create(gtk.join("settings.ini")).context("failed to open settings.ini")?;
            // TODO: Allow customization
            file.write_all(b"[Settings]\ngtk-theme-name=Dracula\n")
                .context("failed to write out settings.ini")?;
            file.flush().context("failed to flush")?;
            file.sync_all().context("failed to sync")?;
        }

        Ok(packager)
    }
}

/// Info needed to run a `cargo build`
pub struct BuildData {
    /// The target triple
    pub target: String,

    /// The build profile
    pub profile: String,

    /// The target binary
    pub bin: String,
}

impl BuildData {
    /// Get the binary file name.
    pub fn get_bin_name(&self) -> String {
        // We assume the user is targeting windows and add an `.exe`
        // as it is not possible to get here with a non-windows without erroring out.
        format!("{}.exe", self.bin)
    }
}

fn main() -> anyhow::Result<()> {
    let options: Options = argh::from_env();
    let ctx = Context::new()?;

    match options.subcommand {
        Subcommand::Build(options) => {
            crate::commands::build::exec(ctx, options)?;
        }
        Subcommand::Package(options) => {
            crate::commands::package::exec(ctx, options)?;
        }
    }

    Ok(())
}
