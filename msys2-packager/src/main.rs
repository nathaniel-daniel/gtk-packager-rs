use anyhow::bail;
use anyhow::ensure;
use anyhow::Context;
use camino::Utf8PathBuf;
use msys2_packager::packager::FileFlags;
use msys2_packager::packager::Packager;
use msys2_packager::util::locate_msys2_installation;
use std::path::PathBuf;

#[derive(Debug)]
struct FileOption {
    src: Option<Utf8PathBuf>,
    dest: Utf8PathBuf,
    flags: FileFlags,
}

impl std::str::FromStr for FileOption {
    type Err = anyhow::Error;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        let mut src = None;
        let mut dest = None;
        let mut flags = FileFlags::empty();

        for part in input.split('|') {
            let (key, value) = part
                .split_once('=')
                .with_context(|| format!("missing key/value pair in `{}`", part))?;
            let key = key.trim();
            let value = value.trim();

            match key {
                "src" => {
                    ensure!(src.is_none(), "two src elements detected");
                    src = Some(value);
                }
                "dest" => {
                    ensure!(dest.is_none(), "two dest elements detected");
                    dest = Some(value);
                }
                "flags" => {
                    ensure!(flags.is_empty(), "two flags elements detected");
                    for flag in value.split(',') {
                        match flag {
                            "exe" => {
                                flags |= FileFlags::EXE;
                            }
                            "upx" => {
                                flags |= FileFlags::UPX;
                            }
                            "lib" => {
                                flags |= FileFlags::LIB;
                            }
                            "add_deps" => {
                                flags |= FileFlags::ADD_DEPS;
                            }
                            flag => {
                                bail!("unknown flag `{flag}`");
                            }
                        }
                    }
                }
                key => {
                    bail!("unknown key `{key}`");
                }
            }
        }

        let src = src.map(|v| v.into());
        let dest = dest.context("missing dest").map(|v| v.into())?;

        Ok(Self { src, dest, flags })
    }
}

#[derive(Debug, argh::FromArgs)]
#[argh(description = "a tool to bundle MSYS2 executables")]
struct Options {
    #[argh(option, long = "file", description = "files to add to the package")]
    files: Vec<FileOption>,

    #[argh(switch, description = "whether to upx the binary")]
    upx: bool,

    #[argh(option, long = "out", short = 'o', description = "the output dir")]
    out: PathBuf,
}

fn main() -> anyhow::Result<()> {
    let options: Options = argh::from_env();

    let msys2_installation_location = locate_msys2_installation()?;
    let msys2_environment = std::env::var("MSYSTEM")
        .context("missing `MSYSTEM` env")?
        .parse()
        .context("invalid MSYSTEM var")?;

    let mut packager = Packager::new(msys2_installation_location, msys2_environment, options.out);
    packager.upx(options.upx);
    for file_option in options.files {
        packager.add_file(
            file_option.src.map(|src| src.into()),
            file_option.dest.into(),
            file_option.flags,
        );
    }

    packager.package().context("failed to package")?;

    Ok(())
}
