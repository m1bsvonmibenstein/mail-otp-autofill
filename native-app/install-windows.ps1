# Installs the Mail OTP Autofill native host + GUI (current user, no admin).
#  - builds the release binaries
#  - registers the native-messaging host for Chrome, Edge, and Firefox
#  - creates a Start Menu shortcut to the account-manager GUI
#
# Usage:
#   powershell -ExecutionPolicy Bypass -File .\install-windows.ps1
#   # override the extension identity if you publish to a store:
#   .\install-windows.ps1 -ChromeExtensionId <id> -FirefoxExtensionId <gecko-id>

param(
  [string]$ChromeExtensionId = "bleebkmhbndbppdebamiidfilopenokn",
  [string]$FirefoxExtensionId = "mail-otp-autofill@mibs"
)

$ErrorActionPreference = "Stop"
$hostName = "com.mibs.otp_relay"
$root = Split-Path -Parent $MyInvocation.MyCommand.Path

# The browser may hold otp-relay.exe open; stop it so the build can replace it.
Get-Process otp-relay, otp-relay-gui -ErrorAction SilentlyContinue | Stop-Process -Force
Start-Sleep -Milliseconds 400

Write-Host "Building (release)..."
Push-Location $root
try {
  cargo build --release
} catch {
  Write-Host "Build failed. If it says 'Access is denied' on otp-relay.exe, the browser"
  Write-Host "re-spawned the host: set the extension's code source to 'Webmail tab' (or"
  Write-Host "close the browser), then run this script again."
  Pop-Location
  exit 1
}
Pop-Location

$exe = Join-Path $root "target\release\otp-relay.exe"
$gui = Join-Path $root "target\release\otp-relay-gui.exe"
if (-not (Test-Path $exe)) { throw "Build did not produce $exe" }

# --- native-messaging host manifests + registry ---------------------------
$manifestDir = Join-Path $root "host-manifest"
New-Item -ItemType Directory -Force -Path $manifestDir | Out-Null
$chromeManifest = Join-Path $manifestDir "$hostName.chrome.json"
$firefoxManifest = Join-Path $manifestDir "$hostName.firefox.json"
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

# --- Start Menu shortcut to the GUI ---------------------------------------
if (Test-Path $gui) {
  $programs = [Environment]::GetFolderPath("Programs")
  $lnk = Join-Path $programs "Mail OTP Autofill.lnk"
  $ws = New-Object -ComObject WScript.Shell
  $sc = $ws.CreateShortcut($lnk)
  $sc.TargetPath = $gui
  $sc.WorkingDirectory = Split-Path -Parent $gui
  $sc.Description = "Manage Mail OTP Autofill accounts"
  $sc.Save()
  Write-Host "  Start Menu shortcut: $lnk"
}

Write-Host ""
Write-Host "Done."
Write-Host "  Host:  $exe"
Write-Host "  GUI:   $gui   (also in Start Menu: 'Mail OTP Autofill')"
Write-Host ""
Write-Host "Next: open the GUI to add a mailbox, or use the CLI:"
Write-Host "  $exe add --label mailcow --host mail.example.com --user you@example.com"
Write-Host "Then set the extension's code source to 'Native app'."
