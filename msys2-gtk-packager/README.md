# msys2-gtk-packager
A specialized packager to bundle gtk-rs projects for Windows using MSYS2. 

## Limitations
 * This only works on Windows.
 * This likely only works for the ucrt64 or mingw64 targets
 * This is over-aggressive and bundles too many dlls.
 * This must be run from an MSYS2 shell, specifically the one that you are attempting to target.
 * You must have the relavent packages installed, which are at least gtk, gstreamer, and a few others.
 * This only works for gtk4.