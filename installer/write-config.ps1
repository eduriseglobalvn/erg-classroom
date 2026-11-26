# Run by the MSI (deferred, SYSTEM). Writes %ProgramData%\ERGClassroom\{room.key,config.txt,agent.path}.
# Same order as install-student-machine.ps1: empty file -> lock ACL -> only then put the key in it.
param([string]$Key, [string]$Room = 'default', [string]$Gateway = '', [string]$Agent, [string]$AllowPower = '', [string]$Upgrade = '', [switch]$Remove)
$ErrorActionPreference = 'Stop'
$data = Join-Path $env:ProgramData 'ERGClassroom'
if ($Remove) { Remove-Item $data -Recurse -Force -ErrorAction SilentlyContinue; return }
# Upgrade: the MSI (KeepConfig) saved the old room.key and config.txt here before the old version deleted the data folder.
# They win over the command line, so an update never moves a computer to another room. Ignored on a fresh install.
$keep = "$data.keep"
# no saved copy (the old version kept its data folder): the files still in place win the same way
if ($Upgrade -and -not (Test-Path "$keep\room.key") -and (Test-Path "$data\room.key")) { $keep = $data }
if ($Upgrade -and (Test-Path "$keep\room.key")) { $Key = (Get-Content "$keep\room.key" -Raw).Trim() }
# no ROOMKEY given: the built-in key, same bytes as DEFAULT_ROOM_KEY in crates\net\src\lib.rs
if (-not $Key) { $Key = -join ([Text.Encoding]::ASCII.GetBytes('ERG Classroom default room key 1') | ForEach-Object { $_.ToString('x2') }) }
if ($Key -notmatch '^[0-9a-fA-F]{64}$') { throw 'ROOMKEY must be exactly 64 hex characters.' }
if ($Gateway -and $Gateway -notmatch '^[\w\.\-]+:\d{2,5}$') { throw 'GATEWAY must look like host:port.' }
if ($Room -match "[\r\n=]") { throw 'ROOM must not contain line breaks or "=".' }
$sys = '*S-1-5-18'; $adm = '*S-1-5-32-544'; $usr = '*S-1-5-32-545'
New-Item -ItemType Directory -Force $data | Out-Null
icacls $data /inheritance:r /grant:r "${sys}:(OI)(CI)F" "${adm}:(OI)(CI)F" "${usr}:(OI)(CI)RX" | Out-Null
if ($LASTEXITCODE) { throw 'ACL of data folder failed.' }
$kf = Join-Path $data 'room.key'
if (Test-Path $kf) { Remove-Item $kf -Force }
New-Item -ItemType File -Path $kf | Out-Null
icacls $kf /inheritance:r /grant:r "${sys}:(R)" "${adm}:(F)" | Out-Null
if ($LASTEXITCODE) { Remove-Item $kf -Force; throw 'ACL of room.key failed.' }
Set-Content -Path $kf -Value $Key -NoNewline -Encoding ascii
$bad = (Get-Acl $kf).Access | Where-Object { $_.IdentityReference.Translate([Security.Principal.SecurityIdentifier]).Value -notin 'S-1-5-18','S-1-5-32-544' }
if ($bad) { Remove-Item $kf -Force; throw 'room.key ACL is too open.' }
$cfg = "room=$Room`r`n"; if ($Gateway) { $cfg += "gateway=$Gateway`r`n" }
if ($AllowPower -eq '1') { $cfg += "allow_power=1`r`n" }
if ($Upgrade -and (Test-Path "$keep\config.txt")) { $cfg = [IO.File]::ReadAllText("$keep\config.txt") }
if ($keep -ne $data) { Remove-Item $keep -Recurse -Force -ErrorAction SilentlyContinue }
# not Set-Content -Encoding utf8: Windows PowerShell adds a BOM, which older agents read as part of the name "room"
[IO.File]::WriteAllText((Join-Path $data 'config.txt'), $cfg)
Set-Content -Path (Join-Path $data 'agent.path') -Value $Agent -Encoding ascii
