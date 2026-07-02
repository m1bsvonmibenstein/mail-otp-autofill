# Builds the native host and registers its native-messaging manifest for
# Chrome, Edge, and Firefox (current user only, no admin needed).
#
# Usage:
#   powershell -ExecutionPolicy Bypass -File .\install-windows.ps1
#   ...optionally override the extension identity if you publish to a store:
#   .\install-windows.ps1 -ChromeExtensionId <id> -FirefoxExtensionId <gecko-id>

param(
  [string]$ChromeExtensionId = "bleebkmhbndbppdebamiidfilopenokn",
  [string]$FirefoxExtensionId = "mail-otp-autofill@mibs"
)

$ErrorActionPreference = "Stop"
$hostName = "com.mibs.otp_relay"
$root = Split-Path -Parent $MyInvocation.MyCommand.Path

Write-Host "Building native host (release)..."
Push-Location $root
cargo build --release
Pop-Location

$exe = Join-Path $root "target\release\otp-relay.exe"
if (-not (Test-Path $exe)) { throw "Build did not produce $exe" }

$manifestDir = Join-Path $root "host-manifest"
New-Item -ItemType Directory -Force -Path $manifestDir | Out-Null
$chromeManifest = Join-Path $manifestDir "$hostName.chrome.json"
$firefoxManifest = Join-Path $manifestDir "$hostName.firefox.json"

# JSON needs escaped backslashes in the Windows path.
$exeJson = $exe.Replace("\", "\\")

@"
{
  "name": "$hostName",
  "description": "Mail OTP Autofill native host",
  "path": "$exeJson",
  "type": "stdio",
  "allowed_origins": ["chrome-extension://$ChromeExtensionId/"]
}
"@ | Out-File -FilePath $chromeManifest -Encoding ascii

@"
{
  "name": "$hostName",
  "description": "Mail OTP Autofill native host",
  "path": "$exeJson",
  "type": "stdio",
  "allowed_extensions": ["$FirefoxExtensionId"]
}
"@ | Out-File -FilePath $firefoxManifest -Encoding ascii

function Register-Host($regPath, $manifestPath) {
  New-Item -Path $regPath -Force | Out-Null
  Set-ItemProperty -Path $regPath -Name "(default)" -Value $manifestPath
  Write-Host "  registered: $regPath"
}

Write-Host "Registering native-messaging host..."
Register-Host "HKCU:\Software\Google\Chrome\NativeMessagingHosts\$hostName" $chromeManifest
Register-Host "HKCU:\Software\Microsoft\Edge\NativeMessagingHosts\$hostName" $chromeManifest
Register-Host "HKCU:\Software\Mozilla\NativeMessagingHosts\$hostName" $firefoxManifest

Write-Host ""
Write-Host "Done. Host '$hostName' -> $exe"
Write-Host "Chrome/Edge extension id: $ChromeExtensionId"
Write-Host "Firefox extension id:     $FirefoxExtensionId"
Write-Host ""
Write-Host "Spike test: set the extension's code source to 'Native app', then watch"
Write-Host "  $($env:TEMP)\otp_relay_spike.log"
Write-Host "Leave the browser idle 3-5 min with no new mail and see if the process"
Write-Host "stays alive (one SPAWNED line) or thrashes (repeated SPAWNED/EXIT ~30s apart)."
