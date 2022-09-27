use anyhow::ensure;

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