/// Possible MSYS2 environments
#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
pub enum Msys2Environment {
    /// This is not recommended,
    /// as it requires the user to have a MSYS2 installation.
    Msys,

    /// This is not recommended if you can target [`Ucrt64`],
    /// as this may be deprecated in the future.
    Mingw64,

    /// This is the recommended target for x86_64.
    Ucrt64,

    Clang64,

    /// This is the recommended target for i686
    Mingw32,

    Clang32,

    /// This is the recommended target for aarch64.
    ClangArm64,
}

impl Msys2Environment {
    /// Get the path prefix.
    pub fn get_prefix(self) -> &'static str {
        match self {
            Self::Msys => "/usr",
            Self::Mingw64 => "/mingw64",
            Self::Ucrt64 => "/ucrt64",
            Self::Clang64 => "/clang64",
            Self::Mingw32 => "/mingw32",
            Self::Clang32 => "/clang32",
            Self::ClangArm64 => "/clangarm64",
        }
    }

    /// Get the arch of the environment
    pub fn get_arch(self) -> Msys2Arch {
        match self {
            Self::Msys | Self::Mingw64 | Self::Ucrt64 | Self::Clang64 => Msys2Arch::X86_64,
            Self::Mingw32 | Self::Clang32 => Msys2Arch::I686,
            Self::ClangArm64 => Msys2Arch::AArch64,
        }
    }
}

/// The architecture of an MSYS2 environment
#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
pub enum Msys2Arch {
    X86_64,
    I686,
    AArch64,
}
