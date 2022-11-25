use crate::util::get_dll_imports;
use crate::util::is_api_set_dll;
use crate::util::is_system_dll;
use crate::util::upx;
use anyhow::bail;
use anyhow::ensure;
use anyhow::Context;
use camino::Utf8PathBuf;
use msys2::Msys2Environment;
use std::collections::HashSet;
use std::ffi::OsStr;
use std::ffi::OsString;
use std::path::Path;
use std::path::PathBuf;

bitflags::bitflags! {
    /// File data
    pub struct FileFlags: u32 {
        /// This file is a dynamic library.
        const LIB = 1 << 0;

        /// This can be upx-ed.
        const UPX = 1 << 1;

        /// This is a runnable executable.
        const EXE = 1 << 2;

        /// Whether to locate and add the binary dependencies of this file automatically.
        const ADD_DEPS = 1 << 3;
    }
}

/// A file to be added to the project.
#[derive(Debug)]
struct File {
    /// The file source.
    ///
    /// If this is none,
    // flags must have either the EXE or LIB attribute.
    // Additionally, dest must be composed of a single component.
    src: Option<PathBuf>,

    /// The file destination.
    ///
    /// This is a relative path, whose base it the package top level.
    dest: PathBuf,

    /// Flags that specify the type of file.
    flags: FileFlags,
}

/// A packager for a GTK-rs project, backed by MSYS2.
pub struct Packager {
    msys2_installation_path: Utf8PathBuf,
    out_dir: PathBuf,
    msys2_environment: Msys2Environment,

    files: Vec<File>,

    resolve_unknown_libraries: bool,
    upx: bool,
}

impl Packager {
    /// Make a new [`Packager`].
    pub fn new(
        msys2_installation_path: Utf8PathBuf,
        msys2_environment: Msys2Environment,
        out_dir: PathBuf,
    ) -> Self {
        Self {
            msys2_installation_path,
            msys2_environment,
            out_dir,

            files: Vec::with_capacity(256),
            resolve_unknown_libraries: true,
            upx: false,
        }
    }

    /// Add a file to be packaged.
    pub fn add_file(&mut self, src: Option<PathBuf>, dest: PathBuf, flags: FileFlags) -> &mut Self {
        self.files.push(File { src, dest, flags });
        self
    }

    /// Whether to resolve unknown libraries.
    ///
    /// Defaults to true.
    pub fn resolve_unknown_libraries(&mut self, resolve_unknown_libraries: bool) -> &mut Self {
        self.resolve_unknown_libraries = resolve_unknown_libraries;
        self
    }

    /// Whether to use upx
    pub fn upx(&mut self, upx: bool) -> &mut Self {
        self.upx = upx;
        self
    }

    /// Get the MSYS2 environment path
    pub fn get_msys2_environment_path(&self) -> Utf8PathBuf {
        self.msys2_installation_path
            .join(self.msys2_environment.get_prefix().trim_start_matches('/'))
    }

    /// Lookup a library with the given packager settings.
    ///
    /// # Result
    /// Returns an error if the library could not be found of if the lookup failed.
    fn lookup_msys2_file(&self, name: &OsStr) -> anyhow::Result<Option<PathBuf>> {
        const PATH_EXT: &[&str] = &["dll", "exe"];

        let lookup_dir = self.get_msys2_environment_path();

        for path in ["lib", "bin"] {
            let path = lookup_dir.join(path);
            let path = path.join_os(name);

            if path
                .try_exists()
                .context("failed to check if file exists")?
            {
                return Ok(Some(path));
            }

            for ext in PATH_EXT {
                // Append .ext to path.
                // Path cannot do this but OsString can.
                let path = {
                    let mut path = OsString::from(&path);
                    path.push(".");
                    path.push(ext);

                    PathBuf::from(path)
                };

                if path
                    .try_exists()
                    .context("failed to check if file exists")?
                {
                    return Ok(Some(path));
                }
            }
        }

        Ok(None)
    }

    // TODO: Consider adding multithreading option.
    /// Try to package
    pub fn package(&mut self) -> anyhow::Result<()> {
        // Create base dir
        std::fs::create_dir_all(&self.out_dir).context("failed to create out dir")?;

        // Lookup missing
        for i in 0..self.files.len() {
            let file = &self.files[i];

            if file.src.is_none() {
                let mut components_iter = file.dest.components();
                let component = components_iter
                    .next()
                    .with_context(|| format!("`{}` has no components", file.dest.display()))?;
                ensure!(
                    components_iter.next().is_none(),
                    "`{}` is longer than 1 component",
                    file.dest.display()
                );

                let name = match component {
                    std::path::Component::Normal(name) => name,
                    _ => bail!("`{}` is not a valid dll name", file.dest.display()),
                };

                let src = self
                    .lookup_msys2_file(name)
                    .with_context(|| format!("failed to locate {:?}", name))?
                    .with_context(|| format!("missing {:?}", name))?;

                eprintln!("Resolved `{}` to `{}`", file.dest.display(), src.display());
                self.files[i].src = Some(src);
            }
        }

        if self.resolve_unknown_libraries {
            let mut known_libraries = HashSet::<OsString>::new();
            let mut unknown_libraries = HashSet::<OsString>::new();
            let mut files_to_copy_offset = 0;
            loop {
                for file in self.files[files_to_copy_offset..].iter().filter(|file| {
                    file.flags.contains(FileFlags::LIB) || file.flags.contains(FileFlags::EXE)
                }) {
                    let file_src = file.src.as_ref().unwrap_or_else(|| {
                        panic!(
                            "`{}` should be resolved, but it is not",
                            file.dest.display()
                        )
                    });
                    let file_name = file_src.file_name().context("missing file name")?;
                    known_libraries.insert(file_name.into());
                    unknown_libraries.remove(file_name);

                    for name in get_dll_imports(file_src)
                        .with_context(|| {
                            format!("failed to get bin deps for `{}`", file_src.display())
                        })?
                        .into_iter()
                        .filter(|name| !is_system_dll(name))
                    {
                        if !known_libraries.contains(OsStr::new(&name)) {
                            if is_api_set_dll(&name) {
                                eprintln!("`{name}` is part of an api set, skipping...");
                                known_libraries.insert(name.into());
                            } else {
                                unknown_libraries.insert(name.into());
                            }
                        }
                    }
                }
                files_to_copy_offset = self.files.len().saturating_sub(1);

                let has_unknown = !unknown_libraries.is_empty();
                for library in unknown_libraries.drain() {
                    let src = self
                        .lookup_msys2_file(&library)
                        .with_context(|| {
                            format!("failed to locate `{}`", Path::new(&library).display())
                        })?
                        .with_context(|| format!("missing `{}`", Path::new(&library).display()))?;

                    println!(
                        "Adding new library `{}` from `{}`...",
                        Path::new(&library).display(),
                        src.display()
                    );
                    self.add_file(
                        Some(src),
                        library.into(),
                        FileFlags::UPX | FileFlags::LIB | FileFlags::ADD_DEPS,
                    );
                }

                if !has_unknown {
                    break;
                }
            }
        }

        for file in self.files.iter() {
            ensure!(
                file.dest.is_relative(),
                "`{}` is an absolute path",
                file.dest.display()
            );

            let file_src = file.src.as_ref().unwrap_or_else(|| {
                panic!(
                    "`{}` should be resolved, but it is not",
                    file.dest.display()
                )
            });
            ensure!(
                !PathBuf::from(OsString::from(file_src).to_ascii_lowercase())
                    .starts_with("c:/windows"),
                "`{}` is being added from a system directory",
                file_src.display()
            );
            let dest = self.out_dir.join(&file.dest);

            // Only attempt a copy if the destination is empty.
            // TODO: Consider emitting a warning if this would cause an overwrite for another file made by this packager.
            if !dest.exists() {
                // Try to create parent dir.
                if let Some(parent) = dest.parent() {
                    std::fs::create_dir_all(parent).with_context(|| {
                        format!("failed to create parent dir at `{}`", parent.display())
                    })?;
                }

                // Perform copy
                std::fs::copy(file_src, &dest).with_context(|| {
                    format!(
                        "failed to copy `{}` to `{}`",
                        file_src.display(),
                        dest.display()
                    )
                })?;

                // If this file is a library or exe and the user asked us to upx it, upx it.
                if self.upx
                    && file.flags.contains(FileFlags::UPX)
                    && (file.flags.contains(FileFlags::LIB) || file.flags.contains(FileFlags::EXE))
                {
                    upx(&dest).with_context(|| format!("failed to upx `{}`", dest.display()))?;
                }
            }
        }

        Ok(())
    }
}
