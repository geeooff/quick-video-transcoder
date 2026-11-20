# Adds "Quick Video Transcoder" to Explorer's Send to menu: select captures, right-click, Send to, and they
# open in the queue.
#
#   .\install-sendto.ps1                      # the quick-video-transcoder.exe next to this script
#   .\install-sendto.ps1 -Exe 'D:\Tools\quick-video-transcoder.exe'
#   .\install-sendto.ps1 -Remove
#
# It writes one shortcut in your own Send to folder and nothing else; no administrator rights.

param(
    [string] $Exe = (Join-Path $PSScriptRoot 'quick-video-transcoder.exe'),
    [switch] $Remove
)

$ErrorActionPreference = 'Stop'
$sendTo = [Environment]::GetFolderPath('SendTo')
$shortcut = Join-Path $sendTo 'Quick Video Transcoder.lnk'

if ($Remove) {
    if (Test-Path $shortcut) {
        Remove-Item $shortcut
        Write-Host "Removed $shortcut"
    } else {
        Write-Host "No shortcut to remove."
    }
    exit 0
}

if (-not (Test-Path $Exe -PathType Leaf)) {
    throw "$Exe not found: pass -Exe with the path of quick-video-transcoder.exe"
}
$Exe = (Resolve-Path $Exe).Path
$shell = New-Object -ComObject WScript.Shell
$link = $shell.CreateShortcut($shortcut)
$link.TargetPath = $Exe
$link.WorkingDirectory = Split-Path -Parent $Exe
$link.Description = 'Trim and re-encode videos to fit a size limit'
$link.Save()
Write-Host "Created $shortcut -> $Exe"
