# msys2-gtk-packager
A specialized packager to bundle gtk-rs projects for Windows using MSYS2. 

## Limitations
 * This only works on Windows, as that is the only platform where you can install MSYS2.
   * Note that this *might* work under WINE, though no testing is currently performed for that platform.
   * Also note that it might be possible to work around this by directly accessing MSYS2's packages, whether though this tool or a fake environment setup tool.
 * This tool forces the use of `msys2`'s pkg-config implementation.
 * This tool is over-aggressive and bundles too many dlls.
 * This only works for gtk4.
 * You must have the relavent packages pre-installed, which are at least gtk4, gstreamer, pkgconfig, and a few others.
 * No testing is performed for targets that are not mingw64.
 * The theme must be "Dracula" (sorry)