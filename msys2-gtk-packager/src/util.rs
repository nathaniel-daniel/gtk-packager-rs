/// LDD utils
mod ldd;

pub use self::ldd::LddIterLibraryDependencies;
pub use self::ldd::LibraryDependency;
use anyhow::bail;
use anyhow::ensure;
use anyhow::Context;
use std::ffi::OsStr;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;

/// A simple function to replicate which
pub fn which(file: &OsStr) -> anyhow::Result<Option<PathBuf>> {
    let path = match std::env::var_os("PATH") {
        Some(var) => var,
        None => {
            return Ok(None);
        }
    };

    // TODO: I think this is irrelavent outside of windows, use feature gate?
    let path_ext = match std::env::var_os("PATHEXT") {
        Some(var) => std::env::split_paths(&var).collect(),
        None => Vec::new(),
    };

    for mut path in std::env::split_paths(&path) {
        path.push(file);

        if path
            .try_exists()
            .with_context(|| format!("failed to check if `{}` exists", path.display()))?
        {
            return Ok(Some(path));
        }

        // TODO: Consider multithreading if user requests it
        for path_ext in path_ext.iter() {
            let mut path = PathBuf::from(&path).into_os_string();
            path.push(path_ext);
            let path = PathBuf::from(path);

            if path
                .try_exists()
                .with_context(|| format!("failed to check if `{}` exists", path.display()))?
            {
                return Ok(Some(path));
            }
        }
    }

    Ok(None)
}

/// Upx a file
pub fn upx<P>(path: P) -> anyhow::Result<()>
where
    P: AsRef<Path>,
{
    let output = Command::new("upx")
        .arg(path.as_ref())
        .arg("--lzma")
        .output()
        .context("failed to run upx")?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("upx exit code was non-zero, `{}`", stderr);
    }

    Ok(())
}

/// Convert an msys2 style path to a Windows path
pub fn msys2_to_windows<P>(path: P) -> anyhow::Result<String>
where
    P: AsRef<Path>,
{
    let output = Command::new("cygpath")
        .arg("-wa")
        .arg(path.as_ref())
        .output()
        .context("failed to run cygpath")?;

    ensure!(output.status.success());
    let mut path = String::from_utf8(output.stdout).context("cygpath output was not utf8")?;
    if path.ends_with("\r\n") {
        path.pop();
        path.pop();
    } else if path.ends_with("\n") {
        path.pop();
    }

    Ok(path)
}
