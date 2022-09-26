use anyhow::bail;
use anyhow::ensure;
use anyhow::Context;
use camino::Utf8PathBuf;
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

/// Upx a file.
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
    } else if path.ends_with('\n') {
        path.pop();
    }

    Ok(path)
}

/// Locate a MSYS2 installation.
///
/// # Returns
/// Returns a [`Utf8PathBuf`].
/// This is because MSYS2 requires an ASCII installation path.
pub fn locate_msys2_installation() -> anyhow::Result<Utf8PathBuf> {
    let mut command = Command::new("cmd");
    let mut output = command
        .arg("/C")
        .arg("msys2 -c \'cygpath -wa /\'")
        .output()
        .with_context(|| format!("failed to spawn `{command:?}`"))?;

    if output.stderr.ends_with(b"\r\n") {
        output.stderr.pop();
        output.stderr.pop();
    }
    if output.stderr.ends_with(&[b'\n']) {
        output.stderr.pop();
    }
    if output.stdout.ends_with(b"\r\n") {
        output.stdout.pop();
        output.stdout.pop();
    }
    if output.stdout.ends_with(&[b'\n']) {
        output.stdout.pop();
    }

    if !output.status.success() {
        bail!(
            "`{command:?}` exited with a non-zero exit code. stderr: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    let path =
        String::from_utf8(output.stdout).context("MSYS2 installation path is not unicode")?;
    Ok(path.into())
}

/// Check if a given dll is a system dll, as in one that is provided by the OS.
pub fn is_system_dll(name: &str) -> bool {
    let name = name.to_ascii_lowercase();
    let name = name.trim_end_matches(".dll");

    matches!(
        name,
        "kernel32"
            | "ole32"
            | "oleaut32"
            | "mfplat"
            | "user32"
            | "mf"
            | "mfreadwrite"
            | "bcrypt"
            | "advapi32"
            | "shell32"
    )
}

/// Get dll imports for the given library or executable.
pub fn get_dll_imports<P>(path: P) -> anyhow::Result<Vec<String>>
where
    P: AsRef<Path>,
{
    let path = path.as_ref();
    let bytes = std::fs::read(&path).context("failed to read file")?;
    let pe = goblin::pe::PE::parse(&bytes).context("failed to parse pe file")?;
    Ok(pe.libraries.iter().map(|name| name.to_string()).collect())
}

/// Check if a given name is an api set dll.
///
/// Names are in the format {api-,ext-}{alphanumeric,'-'}{n}-{n}-{n}.dll where n is a number.
///
/// See https://learn.microsoft.com/en-us/windows/win32/apiindex/windows-apisets.
pub fn is_api_set_dll(name: &str) -> bool {
    // api- exists on all Windows versions, ext- does not.
    if let Some(name) = name
        .strip_prefix("api-")
        .or_else(|| name.strip_prefix("ext-"))
    {
        let name = name.trim_end_matches(".dll");

        let name = match name.rsplit_once('l') {
            Some((name, rest)) => {
                let mut n_iter = rest.split('-').filter(|s| !s.is_empty());
                for _ in 0..3 {
                    let _n = match n_iter.next().map(|n| n.parse::<u32>()) {
                        Some(Ok(n)) => n,
                        None | Some(Err(_)) => {
                            return false;
                        }
                    };
                }

                if n_iter.next().is_some() {
                    return false;
                }

                name
            }
            None => return false,
        };

        for c in name.chars() {
            if !c.is_alphanumeric() && c != '-' {
                return false;
            }
        }

        true
    } else {
        false
    }
}
