$ErrorActionPreference = 'Stop'
$projectRoot = $PSScriptRoot
$launcher = Join-Path $projectRoot 'launch.ps1'
$iconPath = Join-Path $projectRoot 'app.ico'
if (-not (Test-Path -LiteralPath $launcher)) { throw "Missing launcher: $launcher" }
if (-not (Test-Path -LiteralPath $iconPath)) { throw "Missing icon: $iconPath" }

$desktop = [Environment]::GetFolderPath('Desktop')
$shortcutPath = Join-Path $desktop 'Reminisu Economy.lnk'
$shell = New-Object -ComObject WScript.Shell
$shortcut = $shell.CreateShortcut($shortcutPath)
$shortcut.TargetPath = Join-Path $env:SystemRoot 'System32\WindowsPowerShell\v1.0\powershell.exe'
$shortcut.Arguments = '-NoProfile -STA -ExecutionPolicy Bypass -WindowStyle Hidden -File "' + $launcher + '"'
$shortcut.WorkingDirectory = $projectRoot
$shortcut.IconLocation = $iconPath + ',0'
$shortcut.Description = 'Build the latest local code and open Reminisu Economy'
$shortcut.WindowStyle = 7
$shortcut.Save()
Write-Output "Installed: $shortcutPath"
