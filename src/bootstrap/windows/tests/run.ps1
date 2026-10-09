$ErrorActionPreference = 'Stop'
$root = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '../../../..'))
$output = Join-Path $root 'build/startup-gate-tests'
New-Item -ItemType Directory -Force -Path $output | Out-Null
$vswhere = Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio/Installer/vswhere.exe'
$vs = & $vswhere -latest -products * -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
$dev = Join-Path $vs 'Common7/Tools/Launch-VsDevShell.ps1'
& $dev -Arch amd64 -HostArch amd64 -SkipAutomaticLocation | Out-Null
Push-Location $output
try {
    & cl /nologo /std:c++20 /EHsc /MT /LD (Join-Path $PSScriptRoot 'startup_gate_dll.cpp') /link /OUT:startup_gate_test.dll /IMPLIB:startup_gate_test.lib
    if ($LASTEXITCODE) { throw 'Gate DLL build failed' }
    & cl /nologo /std:c++20 /EHsc /MT (Join-Path $PSScriptRoot 'startup_gate_exe.cpp') startup_gate_test.lib /link /OUT:startup_gate_test.exe
    if ($LASTEXITCODE) { throw 'Gate executable build failed' }
    foreach ($fail in @($false, $true)) {
        $start = [Diagnostics.ProcessStartInfo]::new((Join-Path $output 'startup_gate_test.exe'))
        $start.UseShellExecute = $false
        $start.CreateNoWindow = $true
        $start.Environment.Remove('SF_GATE_TEST_FAIL') | Out-Null
        if ($fail) { $start.Environment['SF_GATE_TEST_FAIL'] = '1' }
        $process = [Diagnostics.Process]::Start($start)
        if (-not $process.WaitForExit(10000)) {
            $process.Kill()
            throw 'Startup gate deadlocked'
        }
        $expected = if ($fail) { 1114 } else { 0 }
        if ($process.ExitCode -ne $expected) { throw "Gate test returned $($process.ExitCode), expected $expected" }
        $process.Dispose()
    }
    Write-Host 'Startup gate success and preparation-failure tests passed.'
} finally { Pop-Location }
