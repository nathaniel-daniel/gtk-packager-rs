use anyhow::bail;
use anyhow::Context;
use camino::Utf8PathBuf;
use msys2::Msys2Environment;
use std::path::Path;
use std::process::Command;

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
    let name = name.trim_end_matches(".dll").trim_end_matches(".drv");

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
            | "dnsapi"
            | "gdi32"
            | "imm32"
            | "comdlg32"
            | "opengl32"
            | "shlwapi"
            | "comctl32"
            | "winspool"
            | "version"
            | "cfgmgr32"
            | "kernelbase"
            | "usp10"
            | "msvfw32"
            | "msimg32"
            | "winmm"
            | "rpcrt4"
            | "userenv"
            | "hid"
            | "wsock32"
            | "ntdll"
            | "d3d11"
            | "msvcrt"
            | "gdiplus"
            | "avicap32"
            | "crypt32"
            | "setupapi"
            | "iphlpapi"
            | "ws2_32"
            | "win32u"
            | "ncrypt"
            | "dwmapi"
            | "dxgi"
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

/// Convert a target triple into an MSYS2 environment.
///
/// # Returns
/// Returns an MSYS2 environment.
/// A None value should be taken to mean that the target does not work with MSYS2,
/// however it my be just a flaw in this function.
pub fn target_triple_to_msys2_environment(triple: &str) -> Option<Msys2Environment> {
    // Keep in sync with https://github.com/rust-lang/rust/tree/4d44e09cb1db2788f59159c4b9055e339ed2181d/compiler/rustc_target/src/spec.
    // Just CTRL+F "windows" and ensure all targets present there are present here.
    // Make sure you get the crt right. Look at the link flags to figure it out.
    //
    // I tried to parse these targets but these aren't really "triples".
    // There's no spec or documentation, and people do whatever they want.
    //
    // Generally, -gnullvm targets use UCRT, while gnu use MSVCRT.
    //
    // We cannot support -msvc targets as msys2 provides the wrong library type.
    //
    // We cannot provide i586 as MSYS2 only provides i686.
    //
    // We cannot provide thumb archs as MSYS2 does not provide them.
    //
    // We cannot provide i686 UWP as it is UCRT and MSYS2 only provides x64 UCRT.
    //
    // Clang will always use UCRT.
    match triple {
        "aarch64-pc-windows-gnullvm" => Some(Msys2Environment::ClangArm64),
        "aarch64-pc-windows-msvc" => None,
        "aarch64-uwp-windows-msvc" => None,

        "i586-pc-windows-msvc" => None,

        "i686-pc-windows-gnu" => Some(Msys2Environment::Mingw32),
        "i686-pc-windows-msvc" => None,

        "i686-uwp-windows-gnu" => None,
        "i686-uwp-windows-msvc" => None,

        "thumbv7a-pc-windows-msvc" => None,
        "thumbv7a-uwp-windows-msvc" => None,

        "x86_64-pc-windows-gnu" => Some(Msys2Environment::Mingw64),
        "x86_64-pc-windows-gnullvm" => Some(Msys2Environment::Clang64),
        "x86_64-pc-windows-msvc" => None,

        "x86_64-uwp-windows-gnu" => Some(Msys2Environment::Ucrt64),
        "x86_64-uwp-windows-msvc" => None,
        _ => None,
    }
}
