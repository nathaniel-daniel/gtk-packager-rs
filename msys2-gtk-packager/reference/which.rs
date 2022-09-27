use std::ffi::OsStr;
use std::path::PathBuf;

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