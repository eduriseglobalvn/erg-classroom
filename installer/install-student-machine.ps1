<#
Machine-wide install of the ERG Classroom student. Run as administrator (GPO startup script, Intune, PDQ).
  install-student-machine.ps1 -Agent <erg-student-agent.exe> -Watchdog <erg-student-watchdog.exe> -KeyFile <room.key> `
      [-Room "Phong Tin 1"] [-Gateway "10.0.0.5:47810"]
  install-student-machine.ps1 -Uninstall

What it does
  * copies both programs to %ProgramFiles%\ERGClassroom
  * writes %ProgramData%\ERGClassroom\room.key and config.txt (room, gateway)
  * adds Windows Firewall rules (UDP discovery/Show, QUIC control) for the agent
  * installs the ERGStudent service (LocalSystem, auto start, restarts itself) that keeps the agent running in the
    logged-in user's session, so killing the agent only makes it come back within a few seconds.

Key handling: room.key is created empty, locked to SYSTEM/Administrators, then filled (and the ACL is checked).
The service answers proof requests on the pipe \\.\pipe\ERGClassroomAuth, so the student agent never has the
Room Key (not in a file, not in memory) and cannot forge a tutor.
Privileged actions (web/app/USB/print/Protect filters, power) run in the SYSTEM service: the tutor signs each one with
the Room Key, the agent only forwards it on \\.\pipe\ERGClassroomPriv (ACL: SYSTEM, Administrators, signed-in user).
Power commands (shutdown/restart/log off) are always honoured; -AllowPower is kept only so old command lines still run.
Per-user installs (install-student.ps1) are the weaker mode: the key sits in the user's own folder.
Not run on a lab machine yet.
STATUS: written but not run on a lab machine; test on one PC before deploying (see PLAN.md section 14).
#>
param(
  [string]$Agent, [string]$Watchdog, [string]$KeyFile, [string]$Room = 'default', [string]$Gateway = '', [switch]$TerminalServer, [switch]$AllowPower,
  [switch]$Uninstall
)
$ErrorActionPreference = 'Stop'
if (-not ([Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole('Administrators')) { throw 'Run as administrator.' }
$dest = Join-Path $env:ProgramFiles 'ERGClassroom'
$data = Join-Path $env:ProgramData 'ERGClassroom'
$svc = 'ERGStudent'

function Stop-All {
  if (Get-Service $svc -ErrorAction SilentlyContinue) { Stop-Service $svc -Force -ErrorAction SilentlyContinue }
  Get-Process erg-student-agent -ErrorAction SilentlyContinue | Stop-Process -Force
}

if ($Uninstall) {
  Stop-All
  $wd = Join-Path $dest 'erg-student-watchdog.exe'
  if (Test-Path $wd) { & $wd uninstall }
  Get-NetFirewallRule -DisplayName 'ERG Classroom*' -ErrorAction SilentlyContinue | Remove-NetFirewallRule
  Remove-Item $dest, $data -Recurse -Force -ErrorAction SilentlyContinue
  'Uninstalled.'; return
}

foreach ($f in $Agent, $Watchdog, $KeyFile) { if (-not (Test-Path $f)) { throw "File not found: $f" } }
$hex = (Get-Content $KeyFile -Raw).Trim()
if ($hex -notmatch '^[0-9a-fA-F]{64}$') { throw 'Room key must be exactly 64 hex characters.' }
if ($Gateway -and $Gateway -notmatch '^[\w\.\-]+:\d{2,5}$') { throw 'Gateway must look like host:port.' }
if ($Room -match "[\r\n=]") { throw 'Room name must not contain line breaks or "=".' }

Stop-All
New-Item -ItemType Directory -Force $dest, $data | Out-Null
Copy-Item $Agent (Join-Path $dest 'erg-student-agent.exe') -Force
Copy-Item $Watchdog (Join-Path $dest 'erg-student-watchdog.exe') -Force
$keyFile = Join-Path $data 'room.key'
# SIDs, not names: "SYSTEM"/"Administrators" are translated on non-English Windows.
$sidSystem = '*S-1-5-18'; $sidAdmins = '*S-1-5-32-544'; $sidUsers = '*S-1-5-32-545'
# The data folder keeps no inherited rights: students can read config.txt but cannot change anything in it.
# (The service later opens deploy\ to students for dropping requests, see watchdog deploy_loop.)
icacls $data /inheritance:r /grant:r "${sidSystem}:(OI)(CI)F" "${sidAdmins}:(OI)(CI)F" "${sidUsers}:(OI)(CI)RX" | Out-Null
if ($LASTEXITCODE -ne 0) { throw 'Could not set the ACL of the data folder.' }
# Lock the key file BEFORE the key goes into it, so there is no moment when a student could read it.
if (Test-Path $keyFile) { Remove-Item $keyFile -Force }
New-Item -ItemType File -Path $keyFile | Out-Null
icacls $keyFile /inheritance:r /grant:r "${sidSystem}:(R)" "${sidAdmins}:(F)" | Out-Null
if ($LASTEXITCODE -ne 0) { Remove-Item $keyFile -Force; throw 'Could not set the ACL of room.key.' }
Set-Content -Path $keyFile -Value $hex -NoNewline -Encoding ascii
# Verify: nobody but SYSTEM and Administrators (S-1-5-18 / S-1-5-32-544) may appear in the ACL.
$bad = (Get-Acl $keyFile).Access | Where-Object { $_.IdentityReference.Translate([Security.Principal.SecurityIdentifier]).Value -notin 'S-1-5-18','S-1-5-32-544' }
if ($bad) { Remove-Item $keyFile -Force; throw "room.key is readable by: $($bad.IdentityReference -join ', ')" }
$cfg = "room=$Room`r`n"; if ($Gateway) { $cfg += "gateway=$Gateway`r`n" }
if ($TerminalServer) { $cfg += "terminal_server=1`r`n" }  # one student per signed-in user session
if ($AllowPower) { $cfg += "allow_power=1`r`n" }  # tutor may shut down / restart / log off the machines
Set-Content -Path (Join-Path $data 'config.txt') -Value $cfg -Encoding utf8
# the program folder is only writable by administrators (inherited from Program Files)

$agentExe = Join-Path $dest 'erg-student-agent.exe'
Get-NetFirewallRule -DisplayName 'ERG Classroom*' -ErrorAction SilentlyContinue | Remove-NetFirewallRule
New-NetFirewallRule -DisplayName 'ERG Classroom (agent, inbound)' -Direction Inbound -Action Allow -Program $agentExe -Protocol UDP | Out-Null
New-NetFirewallRule -DisplayName 'ERG Classroom (agent, QUIC)' -Direction Inbound -Action Allow -Program $agentExe -Protocol UDP -Profile Domain,Private | Out-Null

if (Get-Service $svc -ErrorAction SilentlyContinue) { & (Join-Path $dest 'erg-student-watchdog.exe') uninstall }
& (Join-Path $dest 'erg-student-watchdog.exe') install $agentExe
"Installed. The agent starts within a few seconds for the signed-in user."
