<#
Per-user install of the ERG Classroom student (no administrator rights needed).
  install-student.ps1 -Agent <erg-student-agent.exe> -KeyFile <room.key> [-Dest <dir>] [-NoStart]
  install-student.ps1 -Uninstall [-Dest <dir>]
The room key file is the 64-hex-character key the teacher generated; the student never invents one.
WEAKER MODE: everything runs as the student's own user. The Room Key sits in the student's own folder (they can
read it and so could forge a tutor), the agent verifies the tutor-signed policy requests itself, and web/USB
filters need administrator rights so they do not take effect. A student with admin rights can also stop it.
Use install-student-machine.ps1 (SYSTEM service, key locked to SYSTEM/Administrators) for real enforcement.
#>
param(
  [string]$Agent, [string]$KeyFile,
  [string]$Dest = (Join-Path $env:LOCALAPPDATA 'ERGClassroom'),
  [switch]$Uninstall, [switch]$NoStart
)
$ErrorActionPreference = 'Stop'
$run = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Run'
$name = 'ERGClassroomStudent'
Get-Process erg-student-agent -ErrorAction SilentlyContinue | Where-Object { $_.Path -like "$Dest\*" } | Stop-Process -Force
if ($Uninstall) {
  Remove-ItemProperty -Path $run -Name $name -ErrorAction SilentlyContinue
  if (Test-Path $Dest) { Remove-Item $Dest -Recurse -Force }
  'Uninstalled.'; return
}
if (-not (Test-Path $Agent)) { throw "Agent exe not found: $Agent" }
$hex = (Get-Content $KeyFile -Raw).Trim()
if ($hex -notmatch '^[0-9a-fA-F]{64}$') { throw 'Room key must be exactly 64 hex characters.' }
New-Item -ItemType Directory -Force $Dest | Out-Null
Copy-Item $Agent (Join-Path $Dest 'erg-student-agent.exe') -Force
Set-Content -Path (Join-Path $Dest 'room.key') -Value $hex -NoNewline -Encoding ascii
# Only this user may read the key.
icacls (Join-Path $Dest 'room.key') /inheritance:r /grant:r "$($env:USERNAME):(R,W)" | Out-Null
Set-ItemProperty -Path $run -Name $name -Value ('"' + (Join-Path $Dest 'erg-student-agent.exe') + '"')
if (-not $NoStart) { Start-Process (Join-Path $Dest 'erg-student-agent.exe') }
"Installed to $Dest (starts at every logon)."
