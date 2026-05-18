# Agents Notes

## Windows MSVC Build

Run from the repository root in PowerShell:

```powershell
$env:VCPKG_ROOT='C:\Users\mofen\code\vcpkg'
$env:TURBOJPEG_SOURCE='explicit'
$env:TURBOJPEG_LIB_DIR='C:\Users\mofen\code\vcpkg\installed\x64-windows-static\lib'
$env:TURBOJPEG_INCLUDE_DIR='C:\Users\mofen\code\vcpkg\installed\x64-windows-static\include'

cargo build --target x86_64-pc-windows-msvc
```
