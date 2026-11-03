param([ValidateRange(1, 65536)][int]$ParallelJobs = 2)

$ErrorActionPreference = 'Stop'
$root = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..\..'))
$output = Join-Path $root 'build/x64'
$parallelJobCount = [Math]::Min($ParallelJobs, [Environment]::ProcessorCount)
New-Item -ItemType Directory -Force -Path $output | Out-Null
$visualStudio = & (Join-Path $PSScriptRoot 'find_visual_studio.ps1')
$runtimeCmake = Join-Path $visualStudio 'Common7/IDE/CommonExtensions/Microsoft/CMake/CMake/bin/cmake.exe'
& $runtimeCmake -S $root -B (Join-Path $root 'build/native-runtime') -A x64 "-DSHROUDFORGE_NATIVE_COMPILE_JOBS=$parallelJobCount"
if ($LASTEXITCODE -ne 0) { throw 'KFC Runtime configuration failed.' }
& $runtimeCmake --build (Join-Path $root 'build/native-runtime') --config Release --parallel $parallelJobCount
if ($LASTEXITCODE -ne 0) { throw 'KFC Runtime build failed.' }

Copy-Item -LiteralPath (Join-Path $root 'build/native-runtime/bin/kfc-runtime.dll') -Destination $output -Force
