# Builds release exes, then ERGClassroom.msi (Student + Teacher) next to this script.
# Needs: cargo, npm, `wix` (dotnet tool, v5) with the Firewall and UI extensions:
#   wix extension add -g WixToolset.Firewall.wixext/5.0.2
#   wix extension add -g WixToolset.UI.wixext/5.0.2
$ErrorActionPreference = 'Stop'
$root = Split-Path $PSScriptRoot
$env:PATH = "$env:LOCALAPPDATA\dotnet;$env:USERPROFILE\.dotnet\tools;$env:PATH"
Push-Location $root
try {
  $ErrorActionPreference = 'Continue'  # cargo and npm write progress to stderr
  # The Tauri apps embed their web pages only with tauri/custom-protocol; without it the exe looks for the dev server on localhost.
  foreach ($app in @(@('apps\student-agent', 'erg-student-agent'), @('apps\tutor', 'erg-tutor'))) {
    npm --prefix $app[0] run build
    if ($LASTEXITCODE) { throw "npm build of $($app[0]) failed" }
    cargo build --release -p $app[1] --features tauri/custom-protocol
    if ($LASTEXITCODE) { throw "cargo build of $($app[1]) failed" }
  }
  cargo build --release -p erg-student-watchdog
  if ($LASTEXITCODE) { throw 'cargo build failed' }
  $ErrorActionPreference = 'Stop'
  # Offline WebView2 runtime (about 180 MB, kept next to this script): fetched once from Microsoft; only a Microsoft-signed file is packaged.
  $wv2 = Join-Path $PSScriptRoot 'MicrosoftEdgeWebView2RuntimeInstallerX64.exe'
  if (-not (Test-Path $wv2)) { Invoke-WebRequest 'https://go.microsoft.com/fwlink/?linkid=2124701' -OutFile $wv2 -UseBasicParsing }
  $sig = Get-AuthenticodeSignature $wv2
  if ($sig.Status -ne 'Valid' -or $sig.SignerCertificate.Subject -notmatch 'O=Microsoft Corporation') { throw "$wv2 is not signed by Microsoft: delete it and build again." }
  $rel = Join-Path $root 'target\release'
  # the version lives in one place: [workspace.package] in Cargo.toml
  if ((Get-Content "$root\Cargo.toml" -Raw) -notmatch '(?m)^version = "(\d+\.\d+\.\d+)"') { throw 'no version in Cargo.toml' }
  $ver = $Matches[1]
  # one installer for every computer: first install, teacher update and the package the Tutor sends to students
  $out = "$PSScriptRoot\ERGClassroom.msi"
  wix build "$PSScriptRoot\student.wxs" -o $out -d "Version=$ver" -ext WixToolset.Firewall.wixext -ext WixToolset.UI.wixext `
    -bindpath "$rel" -bindpath "$PSScriptRoot" -arch x64
  if ($LASTEXITCODE) { throw 'wix build failed' }
  "Built $out"
} finally { Pop-Location }
