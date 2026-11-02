# Builds the KeyIn TSF DLL, stages librime + Rime data and packages an MSI.
# Requirements: Rust (MSVC toolchain), 7-Zip (7z on PATH), .NET SDK (for the WiX tool).
# Output: build\windows\KeyIn-<version>-windows-<arch>.msi
param(
    [string]$Arch = "x64",
    [string]$RimeVersion = "1.17.0",
    [string]$WixVersion = "5.0.2"
)
$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
$out = Join-Path $root "build\windows"
$stage = Join-Path $out "stage"
$target = @{ x64 = "x86_64-pc-windows-msvc"; arm64 = "aarch64-pc-windows-msvc" }[$Arch]
if (-not $target) { throw "Unsupported architecture: $Arch" }
if ($Arch -ne "x64") { throw "librime publishes MSVC builds for x64 and x86 only; build librime for $Arch first." }
$version = (Select-String -Path (Join-Path $root "windows\Cargo.toml") -Pattern '^version = "(.+)"').Matches[0].Groups[1].Value

Remove-Item -Recurse -Force $stage -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Force $stage, (Join-Path $stage "rime\build"), (Join-Path $out "rime-user") | Out-Null

# librime: the official MSVC release, matching the macOS Homebrew version.
$archive = Join-Path $out "librime-$RimeVersion-$Arch.7z"
if (-not (Test-Path $archive)) {
    $release = Invoke-RestMethod "https://api.github.com/repos/rime/librime/releases/tags/$RimeVersion" -Headers @{ "User-Agent" = "keyin-build" }
    $asset = $release.assets | Where-Object { $_.name -like "rime-*-Windows-msvc-$Arch.7z" -and $_.name -notlike "rime-deps-*" } | Select-Object -First 1
    if (-not $asset) { throw "No librime $RimeVersion MSVC $Arch asset" }
    Invoke-WebRequest $asset.browser_download_url -OutFile $archive
}
$librime = Join-Path $out "librime-$Arch"
Remove-Item -Recurse -Force $librime -ErrorAction SilentlyContinue
7z x $archive "-o$librime" -y | Out-Null
if ($LASTEXITCODE -ne 0) { throw "7z failed" }
Copy-Item (Join-Path $librime "dist\lib\rime.dll") $stage
Copy-Item (Join-Path $librime "dist\lib\rime.dll") (Join-Path $librime "dist\bin")

# Rime schemas and dictionaries, precompiled like scripts/build.sh does on macOS.
$rime = Join-Path $stage "rime"
Copy-Item (Join-Path $root "data\rime\*.yaml"), (Join-Path $root "data\pinyin_simp.dict.yaml") $rime
Get-Content (Join-Path $root "data\rime\keyin-phrases.tsv") -Encoding UTF8 | Add-Content (Join-Path $rime "pinyin_simp.dict.yaml") -Encoding UTF8
& (Join-Path $librime "dist\bin\rime_deployer.exe") --build (Join-Path $out "rime-user") $rime (Join-Path $rime "build")
if ($LASTEXITCODE -ne 0) { throw "rime_deployer failed" }

# The text service DLL.
cargo build --release --locked --manifest-path (Join-Path $root "windows\Cargo.toml") --target $target
if ($LASTEXITCODE -ne 0) { throw "cargo build failed" }
Copy-Item (Join-Path $root "windows\target\$target\release\keyin_tsf.dll") $stage

# Meanings, licenses and notices.
Copy-Item (Join-Path $root "data\chinese-meanings.json") $stage
$notices = Join-Path $stage "ThirdParty"
New-Item -ItemType Directory -Force $notices | Out-Null
Copy-Item (Join-Path $root "LICENSE"), (Join-Path $root "data\LICENSE.*"), (Join-Path $root "data\COPYING.GPL3"), (Join-Path $root "data\README.*.md"), (Join-Path $root "data\chinese-meanings-manifest.json") $notices
Copy-Item (Join-Path $librime "version-info.txt") (Join-Path $notices "librime-version-info.txt")

# MSI via WiX (installed as a .NET tool on first use).
if (-not (Get-Command wix -ErrorAction SilentlyContinue)) {
    dotnet tool install --global wix --version $WixVersion
    $env:PATH += ";$env:USERPROFILE\.dotnet\tools"
}
$msi = Join-Path $out "KeyIn-$version-windows-$Arch.msi"
wix build (Join-Path $root "windows\installer\KeyIn.wxs") -arch $Arch -d "Version=$version" -d "SourceDir=$stage" -o $msi
if ($LASTEXITCODE -ne 0) { throw "wix build failed" }
Write-Host "Built $msi"
