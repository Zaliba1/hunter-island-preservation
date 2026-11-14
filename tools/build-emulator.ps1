param([Parameter(Mandatory=$true)][string]$BoostRoot,
      [Parameter(Mandatory=$true)][string]$GnuRoot,
      [string]$TargetDir = (Join-Path $env:LOCALAPPDATA 'HI-Build'),
      [ValidateRange(1,16)][int]$Jobs = 8,
      [switch]$Offline,
      [switch]$RebuildOpenAL)
$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path $PSScriptRoot -Parent
$engineRoot = Join-Path $projectRoot 'emulator'
$TargetDir = [IO.Path]::GetFullPath($TargetDir)
if ($TargetDir.Length -gt 45) {
    throw 'Use a short Windows target path of at most 45 characters, for example C:/hi-build. Native GCC dependency paths exceed Windows limits in longer output directories.'
}
if ($projectRoot -match '\s' -or $GnuRoot -match '\s') { throw 'Use source/toolchain paths without spaces, for example C:\Build\HunterSource and C:\Build\mingw64.' }
$boostTarget = Join-Path $engineRoot 'vendor\boost'
if (!(Test-Path (Join-Path $BoostRoot 'boost\version.hpp'))) { throw 'BoostRoot must contain boost\version.hpp (Boost 1.81.0).' }
if (!(Test-Path $boostTarget)) { Copy-Item -LiteralPath $BoostRoot -Destination $boostTarget -Recurse }
$gnuBin = Join-Path ([IO.Path]::GetFullPath($GnuRoot)) 'bin'
foreach ($program in @('gcc.exe','g++.exe','ar.exe','cmake.exe','ninja.exe','strip.exe')) {
    if (!(Test-Path (Join-Path $gnuBin $program))) { throw "GNU toolchain is missing $program" }
}
$env:PATH = $gnuBin + ';' + $env:PATH
$env:CC_x86_64_pc_windows_gnu = Join-Path $gnuBin 'gcc.exe'
$env:CXX_x86_64_pc_windows_gnu = Join-Path $gnuBin 'g++.exe'
$env:AR_x86_64_pc_windows_gnu = Join-Path $gnuBin 'ar.exe'
$env:CMAKE = Join-Path $gnuBin 'cmake.exe'
$env:CMAKE_GENERATOR = 'Ninja'
$env:CMAKE_POLICY_VERSION_MINIMUM = '3.5'
$env:CMAKE_BUILD_PARALLEL_LEVEL = [string]$Jobs
$env:CARGO_BUILD_JOBS = [string]$Jobs
$env:CARGO_TARGET_DIR = [IO.Path]::GetFullPath($TargetDir)
$env:CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER = Join-Path $gnuBin 'gcc.exe'
$env:CARGO_PROFILE_RELEASE_DEBUG = '0'
$privacyFlags = @("--remap-path-prefix=$env:USERPROFILE=/build", "--remap-path-prefix=$engineRoot=/source", '-C', 'link-arg=-s', '-C', 'link-arg=-static', '-C', 'link-arg=-static-libgcc', '-C', 'link-arg=-static-libstdc++')
$env:CARGO_ENCODED_RUSTFLAGS = $privacyFlags -join [char]0x1f
Remove-Item Env:RUSTFLAGS -ErrorAction SilentlyContinue
$homeForward = $env:USERPROFILE.Replace('\','/')
$engineForward = $engineRoot.Replace('\','/')
$commonFlags = "-ffile-prefix-map=$env:USERPROFILE=/build -ffile-prefix-map=$engineRoot=/source -ffile-prefix-map=$homeForward=/build -ffile-prefix-map=$engineForward=/source -g0"
# SDL's pinned C sources predate GCC 15's C23 default.
$env:CFLAGS = '-std=gnu11 ' + $commonFlags
$env:CXXFLAGS = $commonFlags
Push-Location -LiteralPath $engineRoot
try {
    $buildFlags = @('--target','x86_64-pc-windows-gnu','--locked','--release')
    if ($Offline) { $env:CARGO_NET_OFFLINE = 'true'; $buildFlags += '--frozen' }
    if ($RebuildOpenAL) {
        cargo +1.90.0-x86_64-pc-windows-gnu clean --target x86_64-pc-windows-gnu --release -p touchHLE_openal_soft_wrapper
        if ($LASTEXITCODE) { throw 'OpenAL build cleanup failed.' }
    }
    cargo +1.90.0-x86_64-pc-windows-gnu test @buildFlags --lib -- --test-threads=1
    if ($LASTEXITCODE) { throw 'Emulator tests failed.' }
    cargo +1.90.0-x86_64-pc-windows-gnu build @buildFlags
    if ($LASTEXITCODE) { throw 'Emulator build failed.' }
} finally { Pop-Location }
# GNU import-library symbol names can contain the compiler's temporary path.
# Strip the distributed executable's COFF/debug symbols explicitly, then audit it.
& (Join-Path $gnuBin 'strip.exe') --strip-all (Join-Path $env:CARGO_TARGET_DIR 'x86_64-pc-windows-gnu\release\touchHLE.exe')
if ($LASTEXITCODE) { throw 'Executable symbol stripping failed.' }
Write-Host (Join-Path $env:CARGO_TARGET_DIR 'x86_64-pc-windows-gnu\release\touchHLE.exe')
