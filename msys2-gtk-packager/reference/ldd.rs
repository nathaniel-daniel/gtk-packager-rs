use anyhow::anyhow;
use anyhow::Context;
use std::ffi::OsStr;
use std::io::BufRead;
use std::io::BufReader;
use std::io::Read;
use std::path::Path;
use std::path::PathBuf;
use std::process::Child;
use std::process::ChildStderr;
use std::process::ChildStdout;
use std::process::Command;
use std::process::Stdio;

/// A library that is a dependency of another library or executable.
#[derive(Debug)]
pub struct LibraryDependency {
    /// The name of the library
    pub name: Box<str>,

    /// The path of the library.
    ///
    /// # Limitations
    /// Currently, this will always be UTF-8
    pub path: PathBuf,
}

impl LibraryDependency {
    /// Whether this is a system library.
    ///
    /// A "system" dll is defined as a dll that is shipped as a part of the operating system.
    /// As an example, this includes everything under `C:/Windows` on Windows machines.
    ///
    /// # Limitations
    /// This does not consider the operating system.
    /// While `/c/windows` is not a system dll, it is still classified as a system dll.
    /// This currently operates by scanning the path.
    /// In the future, it may be implemeted via a static map of dll names.
    ///
    /// This function also will NOT consider case to maintain cross-platform compatability.
    pub fn is_system_library(&self) -> bool {
        // Forcibly ignore case.
        let path_lower = PathBuf::from(OsStr::new(&self.path).to_ascii_lowercase());
        path_lower.starts_with("/c/windows")
    }
}

/// An iterator over dependencies of a library
///
/// This is backed by parsing the output of ldd.
/// # Limitations
/// Currently, all located paths MUST be valid unicode.
/// It is safe to ignore unicode decoding errors to get the next line, though you will skip data.
pub struct LddIterLibraryDependencies {
    child: Child,

    stdout: BufReader<ChildStdout>,
    stderr: ChildStderr,
}

impl LddIterLibraryDependencies {
    /// Look up the dependencies for the library at the given path.
    pub fn new<P>(path: P) -> anyhow::Result<Self>
    where
        P: AsRef<Path>,
    {
        let mut child = Command::new("ldd")
            .arg(path.as_ref())
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .context("failed to spawn ldd")?;
        let _stdin = child.stdin.take();
        let stdout = BufReader::new(child.stdout.take().expect("missing stdout"));
        let stderr = child.stderr.take().expect("missing stderr");

        Ok(Self {
            child,
            stdout,
            stderr,
        })
    }
}

impl Iterator for LddIterLibraryDependencies {
    type Item = anyhow::Result<LibraryDependency>;

    fn next(&mut self) -> Option<Self::Item> {
        let mut line = String::with_capacity(32);
        match self.stdout.read_line(&mut line) {
            Ok(n) if n == 0 => {
                // stdout is empty, drain stderr buffer.
                let mut stderr_buffer = String::new();
                if let Err(e) = self.stderr.read_to_string(&mut stderr_buffer) {
                    return Some(Err(e).context("failed to read stderr"));
                }
                if !stderr_buffer.is_empty() {
                    if stderr_buffer.ends_with("\r\n") {
                        stderr_buffer.pop();
                        stderr_buffer.pop();
                    }
                    if stderr_buffer.ends_with("\n") {
                        stderr_buffer.pop();
                    }
                    return Some(Err(anyhow!("stderr is not empty, `{}`", stderr_buffer)));
                }

                // Wait the child, then return the status if it is not zero or waiting failed.
                match self.child.wait().context("failed to wait for child") {
                    Ok(status) if !status.success() => {
                        return Some(Err(anyhow!("non-zero exit status `{status}`")))
                    }
                    Ok(_status) => {}
                    Err(e) => return Some(Err(e)),
                }

                // TODO: Make iter fused?
                // Signal iter end
                return None;
            }
            Ok(_n) => {
                let (name, path) = match line
                    .split_once("=>")
                    .map(|(name, line)| (name.trim(), line.trim()))
                    .context("line missing `=>`")
                    .and_then(|(name, line)| {
                        let (path, _line) = line.rsplit_once(' ').context("missing path")?;
                        Ok((name, path))
                    }) {
                    Ok(v) => v,
                    Err(e) => {
                        return Some(Err(e));
                    }
                };

                let library = LibraryDependency {
                    name: name.into(),
                    path: path.into(),
                };

                Some(Ok(library))
            }
            Err(e) => {
                return Some(Err(e).context("failed to read line"));
            }
        }
    }
}

impl Drop for LddIterLibraryDependencies {
    fn drop(&mut self) {
        let _ = self.child.wait().is_ok();
    }
}
