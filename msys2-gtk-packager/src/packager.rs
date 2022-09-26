use crate::util::get_dll_imports;
use crate::util::is_api_set_dll;
use crate::util::is_system_dll;
use crate::util::upx;
use anyhow::ensure;
use anyhow::Context;
use std::collections::HashSet;
use std::ffi::OsStr;
use std::ffi::OsString;
use std::path::Path;
use std::path::PathBuf;

use crate::util::msys2_to_windows;
// TODO: Avoid which, use custom lookup impl based on msys2 env type.
use crate::util::which;

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
    out_dir: PathBuf,

    files: Vec<File>,

    resolve_unknown_libraries: bool,
    upx: bool,
}

impl Packager {
    /// Make a new [`Packager`].
    pub fn new(out_dir: PathBuf) -> Self {
        Self {
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

    // TODO: Consider adding multithreading option.
    /// Try to package
    pub fn package(&mut self) -> anyhow::Result<()> {
        // Create base dir
        std::fs::create_dir_all(&self.out_dir).context("failed to create out dir")?;

        for file in self.files.iter_mut() {
            if file.src.is_none() {
                ensure!(file.dest.components().count() == 1);
                let src = which(OsStr::new(&file.dest))
                    .with_context(|| format!("failed to locate `{}`", file.dest.display()))?
                    .with_context(|| format!("missing `{}`", file.dest.display()))?;
                eprintln!("Resolved `{}` to `{}`", file.dest.display(), src.display());
                file.src = Some(src);
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
                    let file_src = file.src.as_ref().expect(&format!(
                        "`{}` should be resolved, but it is not",
                        file.dest.display()
                    ));
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
                    let src = which(&library)
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

            let file_src = file.src.as_ref().expect(&format!(
                "`{}` should be resolved, but it is not",
                file.dest.display()
            ));
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

                // We need to translate the path from MSYS2 to Windows.
                let src =
                    PathBuf::from(msys2_to_windows(file_src).context("failed to convert path")?);

                // Perform copy
                std::fs::copy(&src, &dest).with_context(|| {
                    format!("failed to copy `{}` to `{}`", src.display(), dest.display())
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
