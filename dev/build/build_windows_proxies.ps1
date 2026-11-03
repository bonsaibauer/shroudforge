param([string]$CargoTargetDir = 'target')
$ErrorActionPreference = 'Stop'
$root = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..\..'))
$output = Join-Path $root 'build/x64'
New-Item -ItemType Directory -Force -Path $output | Out-Null
$cargoTargetPath = if ([IO.Path]::IsPathRooted($CargoTargetDir)) { [IO.Path]::GetFullPath($CargoTargetDir) } else { [IO.Path]::GetFullPath((Join-Path $root $CargoTargetDir)) }
& cargo build --locked --manifest-path (Join-Path $root 'kfc-parser\Cargo.toml') --release -p dbghelp-proxy -p dinput8-proxy --target-dir $cargoTargetPath
if ($LASTEXITCODE -ne 0) { throw 'EML-compatible Windows proxy build failed.' }

Copy-Item -LiteralPath (Join-Path $cargoTargetPath 'release/dbghelp.dll'),(Join-Path $cargoTargetPath 'release/dinput8.dll') -Destination $output -Force
