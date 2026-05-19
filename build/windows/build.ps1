param(
    [string]$Configuration = "debug",
    [string]$Target = "x86_64-pc-windows-msvc",
    [string]$Triplet = "x64-windows-static",
    [string]$VcpkgRoot = $env:VCPKG_ROOT,
    [string]$VcpkgInstalledRoot = $env:VCPKG_INSTALLED_DIR,
    [switch]$NoDefaultFeatures,
    [string[]]$Features = @(),
    [switch]$Package,
    [Parameter(ValueFromRemainingArguments = $true)]
    [string[]]$CargoArgs = @()
)

$ErrorActionPreference = "Stop"

$repoRoot = Resolve-Path (Join-Path $PSScriptRoot "..\..")
Set-Location $repoRoot

if ([string]::IsNullOrWhiteSpace($VcpkgRoot)) {
    $VcpkgRoot = Join-Path (Split-Path $repoRoot -Parent) "vcpkg"
}

$VcpkgRoot = [System.IO.Path]::GetFullPath($VcpkgRoot)
if ([string]::IsNullOrWhiteSpace($VcpkgInstalledRoot)) {
    $VcpkgInstalledRoot = Join-Path $VcpkgRoot "installed"
}

$VcpkgInstalledRoot = [System.IO.Path]::GetFullPath($VcpkgInstalledRoot)
$vcpkgTripletRoot = Join-Path $VcpkgInstalledRoot $Triplet
$turbojpegLibDir = Join-Path $vcpkgTripletRoot "lib"
$turbojpegIncludeDir = Join-Path $vcpkgTripletRoot "include"

if (-not (Test-Path $VcpkgRoot)) {
    throw "VCPKG_ROOT does not exist: $VcpkgRoot. Run build/windows/bootstrap-vcpkg.ps1 first."
}

if (-not (Test-Path $turbojpegLibDir) -or -not (Test-Path $turbojpegIncludeDir)) {
    throw "vcpkg triplet is not installed at $vcpkgTripletRoot. Run build/windows/bootstrap-vcpkg.ps1 first."
}

$env:VCPKG_ROOT = $VcpkgRoot
$env:VCPKG_DEFAULT_TRIPLET = $Triplet
$env:VCPKG_INSTALLED_DIR = $VcpkgInstalledRoot
$env:TURBOJPEG_SOURCE = "explicit"
$env:TURBOJPEG_LIB_DIR = $turbojpegLibDir
$env:TURBOJPEG_INCLUDE_DIR = $turbojpegIncludeDir

$cargoCommand = @("build", "--target", $Target)

if ($Configuration -eq "release") {
    $cargoCommand += "--release"
} elseif ($Configuration -ne "debug") {
    throw "Unsupported configuration '$Configuration'. Use 'debug' or 'release'."
}

if ($NoDefaultFeatures) {
    $cargoCommand += "--no-default-features"
}

if ($Features.Count -gt 0) {
    $cargoCommand += "--features"
    $cargoCommand += ($Features -join ",")
}

$cargoCommand += $CargoArgs

cargo @cargoCommand

if ($Package) {
    $metadata = cargo metadata --no-deps --format-version 1 | ConvertFrom-Json
    $packageInfo = $metadata.packages | Where-Object { $_.name -eq "one-kvm" } | Select-Object -First 1

    if ($null -eq $packageInfo -or [string]::IsNullOrWhiteSpace($packageInfo.version)) {
        throw "Failed to resolve version from Cargo metadata"
    }

    $sourcePath = Join-Path $repoRoot "target/$Target/release/one-kvm.exe"
    $targetName = "one-kvm_{0}_amd64.exe" -f $packageInfo.version
    $targetPath = Join-Path $repoRoot "target/$Target/release/$targetName"

    if (-not (Test-Path $sourcePath)) {
        throw "Windows binary not found: $sourcePath"
    }

    Copy-Item $sourcePath $targetPath
    Write-Host $targetPath
}
