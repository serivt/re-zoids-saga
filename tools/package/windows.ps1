# Builds the Windows package: the program and its texts in
# dist/re-zoids-saga-<version>-windows-x86_64.zip. The C runtime is linked
# in, so the program runs without the Visual C++ Redistributable.
# Usage: tools/package/windows.ps1 [version]

param([string]$Version = (git describe --tags --always --dirty))

$ErrorActionPreference = "Stop"
$Executable = "re-zoids-saga"
$Dist = "dist"
$Folder = "$Executable-$Version"
$Stage = Join-Path $Dist "stage"

$env:RUSTFLAGS = "-C target-feature=+crt-static"
cargo build --release --locked -p launcher --features packaged
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

Remove-Item -Recurse -Force $Stage -ErrorAction SilentlyContinue
$Target = New-Item -ItemType Directory -Force (Join-Path $Stage $Folder)
Copy-Item target/release/launcher.exe (Join-Path $Target "$Executable.exe")
Copy-Item LICENSE (Join-Path $Target "LICENSE.txt")
Copy-Item tools/package/README-player.txt (Join-Path $Target "README.txt")

$Archive = Join-Path $Dist "$Executable-$Version-windows-x86_64.zip"
Remove-Item -Force $Archive -ErrorAction SilentlyContinue
Compress-Archive -Path (Join-Path $Stage $Folder) -DestinationPath $Archive
Write-Output $Archive
