param([switch]$BuildOnly)

$ErrorActionPreference = 'Stop'
$projectRoot = $PSScriptRoot
Set-Location -LiteralPath $projectRoot
Add-Type -AssemblyName System.Windows.Forms

# Keep the build and running executable under one lock: Windows cannot replace
# an executable that is still running. Repeated shortcut clicks are harmless.
$mutex = New-Object System.Threading.Mutex($false, 'Local\ReminisuEconomyLauncher')
$locked = $false
$window = $null
try {
    try { $locked = $mutex.WaitOne(0) }
    catch [System.Threading.AbandonedMutexException] { $locked = $true }
    if (-not $locked) {
        if (-not $BuildOnly) {
            [System.Windows.Forms.MessageBox]::Show('Reminisu Economy is already open or updating. Close its window before launching the latest code.', 'Reminisu Economy') | Out-Null
        }
        exit 0
    }

    $cargo = Get-Command cargo.exe -ErrorAction SilentlyContinue
    $cargoPath = if ($cargo) { $cargo.Source } else { Join-Path $env:USERPROFILE '.cargo\bin\cargo.exe' }
    if (-not (Test-Path -LiteralPath $cargoPath)) { throw 'Rust Cargo was not found. Install Rust or restore its installation before launching.' }

    $logDirectory = Join-Path $projectRoot 'target\launcher'
    New-Item -ItemType Directory -Path $logDirectory -Force | Out-Null
    $buildLog = Join-Path $logDirectory 'build.log'
    $outputLog = Join-Path $logDirectory 'build-output.log'
    $appLog = Join-Path $logDirectory 'app.log'

    if (-not $BuildOnly) {
        $window = New-Object System.Windows.Forms.Form
        $window.Text = 'Reminisu Economy'
        $window.ClientSize = New-Object System.Drawing.Size(390, 95)
        $window.StartPosition = 'CenterScreen'
        $window.FormBorderStyle = 'FixedDialog'
        $window.ControlBox = $false
        $label = New-Object System.Windows.Forms.Label
        $label.Text = 'Building the latest version. The app will open shortly.'
        $label.AutoSize = $true
        $label.Location = New-Object System.Drawing.Point(20, 20)
        $window.Controls.Add($label)
        $progress = New-Object System.Windows.Forms.ProgressBar
        $progress.Style = 'Marquee'
        $progress.SetBounds(20, 50, 350, 20)
        $window.Controls.Add($progress)
        $window.Show()
    }

    # Cargo's dependency tracking rebuilds only what changed. An explicit target
    # directory makes the executable location independent of CARGO_TARGET_DIR.
    $build = Start-Process -FilePath $cargoPath -ArgumentList @('build', '--offline', '--locked', '--features', 'gui', '--bin', 'reminisu_gui', '--target-dir', 'target') -WorkingDirectory $projectRoot -WindowStyle Hidden -RedirectStandardError $buildLog -RedirectStandardOutput $outputLog -PassThru
    # Retain the process handle so Windows PowerShell preserves the exit code.
    $null = $build.Handle
    while (-not $build.HasExited) {
        [System.Windows.Forms.Application]::DoEvents()
        Start-Sleep -Milliseconds 100
        $build.Refresh()
    }
    $build.WaitForExit()
    if ($build.ExitCode -ne 0) { throw "The latest code could not be built. Details: $buildLog`n`n$(Get-Content -LiteralPath $buildLog -Tail 12 | Out-String)" }
    if ($window) { $window.Close(); $window.Dispose(); $window = $null }
    if (-not $BuildOnly) {
        $app = Start-Process -FilePath (Join-Path $projectRoot 'target\debug\reminisu_gui.exe') -WorkingDirectory $projectRoot -WindowStyle Hidden -RedirectStandardError $appLog -PassThru
        $null = $app.Handle
        $app.WaitForExit()
        if ($app.ExitCode -ne 0) { throw "The app exited with error $($app.ExitCode). Details: $appLog" }
    }
} catch {
    if ($window) { $window.Close(); $window.Dispose(); $window = $null }
    if ($BuildOnly) { Write-Error $_ }
    else { [System.Windows.Forms.MessageBox]::Show($_.Exception.Message, 'Reminisu Economy - launch failed', 'OK', 'Error') | Out-Null }
    exit 1
} finally {
    if ($locked) { $mutex.ReleaseMutex() }
    $mutex.Dispose()
}
