# SPDX-License-Identifier: MPL-2.0
param([Parameter(Mandatory=$true)][string]$SdkRoot,
      [Parameter(Mandatory=$true)][string]$JavaHome,
      [Parameter(Mandatory=$true)][string]$GradleRoot,
      [Parameter(Mandatory=$true)][string]$GnuRoot,
      [Parameter(Mandatory=$true)][string]$SigningConfig,
      [Parameter(Mandatory=$true)][string]$TargetDir,
      [string]$Python='python',
      [switch]$RebuildOpenAL,
      [switch]$Test,
      [switch]$OfflineGradle)
$ErrorActionPreference='Stop'
$projectRoot=Split-Path $PSScriptRoot -Parent
if($projectRoot -match '\s' -or $SdkRoot -match '\s'){throw 'Use short source and SDK paths without spaces.'}
$SdkRoot=[IO.Path]::GetFullPath($SdkRoot)
$ndkRoot=Join-Path $SdkRoot 'ndk\28.2.13676358'
$llvmBin=Join-Path $ndkRoot 'toolchains\llvm\prebuilt\windows-x86_64\bin'
$env:JAVA_HOME=[IO.Path]::GetFullPath($JavaHome)
$env:ANDROID_HOME=$SdkRoot
$env:ANDROID_SDK_ROOT=$SdkRoot
$env:ANDROID_SDK_USAGE_TRACKING='0'
$env:ANDROID_NDK_HOME=$ndkRoot
$env:ANDROID_NDK_ROOT=$ndkRoot
$env:PATH=(Join-Path $env:JAVA_HOME 'bin')+';'+(Join-Path $GnuRoot 'bin')+';'+$llvmBin+';'+$env:PATH
$env:CMAKE=Join-Path $SdkRoot 'cmake\3.22.1\bin\cmake.exe'
$env:CMAKE_GENERATOR='Ninja'
$env:CMAKE_BUILD_PARALLEL_LEVEL='8'
$env:CARGO_BUILD_JOBS='8'
$env:CARGO_TARGET_DIR=[IO.Path]::GetFullPath($TargetDir)
$env:CARGO_PROFILE_RELEASE_DEBUG='0'
$env:CARGO_NET_OFFLINE='true'
$env:CC_aarch64_linux_android=Join-Path $llvmBin 'clang.exe'
$env:CXX_aarch64_linux_android=Join-Path $llvmBin 'clang++.exe'
$env:AR_aarch64_linux_android=Join-Path $llvmBin 'llvm-ar.exe'
$env:CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER=Join-Path $llvmBin 'aarch64-linux-android23-clang.cmd'
$env:CARGO_ENCODED_RUSTFLAGS=@("--remap-path-prefix=$env:USERPROFILE=/build", "--remap-path-prefix=$projectRoot=/source",'-C','link-arg=-Wl,-z,max-page-size=16384') -join [char]0x1f
$forwardHome=$env:USERPROFILE.Replace('\','/')
$env:CFLAGS_aarch64_linux_android="--target=aarch64-linux-android23 -ffile-prefix-map=$env:USERPROFILE=/build -ffile-prefix-map=$forwardHome=/build -g0"
$env:CXXFLAGS_aarch64_linux_android=$env:CFLAGS_aarch64_linux_android
$build=Join-Path $projectRoot '.build'
New-Item -ItemType Directory -Force -Path $build | Out-Null
$toolchainFile=Join-Path $build 'android-arm64-toolchain.cmake'
$toolchainText="set(ANDROID_ABI arm64-v8a CACHE STRING `"`" FORCE)`nset(ANDROID_PLATFORM android-23 CACHE STRING `"`" FORCE)`nset(ANDROID_STL c++_shared CACHE STRING `"`" FORCE)`ninclude(`"$($ndkRoot.Replace('\','/'))/build/cmake/android.toolchain.cmake`")`n"
[IO.File]::WriteAllText($toolchainFile,$toolchainText)
$env:CMAKE_TOOLCHAIN_FILE=$toolchainFile
$env:CARGO_NDK_SYSROOT_LIBS_PATH=Join-Path $ndkRoot 'toolchains\llvm\prebuilt\windows-x86_64\sysroot\usr\lib\aarch64-linux-android'
New-Item -ItemType Directory -Force -Path (Join-Path $projectRoot 'emulator\target\aarch64-linux-android\release') | Out-Null
& $Python (Join-Path $PSScriptRoot 'patch_android_sdl.py')
if($LASTEXITCODE){throw 'SDL compatibility preparation failed'}
Push-Location -LiteralPath (Join-Path $projectRoot 'emulator')
try {
 if($RebuildOpenAL) {
  cargo +1.90.0-x86_64-pc-windows-gnu clean --target aarch64-linux-android --release -p touchHLE_openal_soft_wrapper
  if($LASTEXITCODE){throw 'Android OpenAL cleanup failed'}
 }
 cargo +1.90.0-x86_64-pc-windows-gnu build --target aarch64-linux-android --release --frozen --lib --no-default-features --features 'touchHLE_openal_soft_wrapper/static,sdl2/bundled'
 if($LASTEXITCODE){throw 'Android native build failed'}
} finally {Pop-Location}
& $Python (Join-Path $PSScriptRoot 'stage_android_libraries.py') --target-dir $TargetDir --sdk-root $SdkRoot
if($LASTEXITCODE){throw 'Android library staging failed'}
$signing=Get-Content -LiteralPath $SigningConfig -Raw | ConvertFrom-Json
$env:HI_ANDROID_SIGNING_FILE=Join-Path (Split-Path ([IO.Path]::GetFullPath($SigningConfig)) -Parent) 'hunter-preview.p12'
$env:HI_ANDROID_SIGNING_PASSWORD=$signing.password
try {
 Push-Location -LiteralPath (Join-Path $projectRoot 'emulator\android')
 try {
  $tasks = @('--no-daemon','--console=plain')
  if ($OfflineGradle) { $tasks += '--offline' }
  if ($Test) { $tasks += ':app:testDebugUnitTest' }
  $tasks += ':app:assembleDebug'
  & (Join-Path $GradleRoot 'bin\gradle.bat') @tasks
  if($LASTEXITCODE){throw 'Android APK build failed'}
 } finally {Pop-Location}
} finally {Remove-Item Env:HI_ANDROID_SIGNING_PASSWORD -ErrorAction SilentlyContinue}
Write-Host (Join-Path $projectRoot 'emulator\android\app\build\outputs\apk\debug\app-debug.apk')
