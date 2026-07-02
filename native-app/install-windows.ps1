# Installs Mail OTP Autofill: always-on daemon + bridge + GUI (current user, no admin).
#  - builds the release binaries
#  - registers the native-messaging host -> bridge (which talks to the daemon)
#  - autostarts the daemon hidden at login, and starts it now
#  - Start Menu shortcut to the account-manager GUI
#
# Usage:
#   powershell -ExecutionPolicy Bypass -File .\install-windows.ps1
#   .\install-windows.ps1 -ChromeExtensionId <id> -FirefoxExtensionId <gecko-id>

param(
  [string]$ChromeExtensionId = "bleebkmhbndbppdebamiidfilopenokn",
  [string]$FirefoxExtensionId = "mail-otp-autofill@mibs"
)

$ErrorActionPreference = "Stop"
$hostName = "com.mibs.otp_relay"
$root = Split-Path -Parent $MyInvocation.MyCommand.Path

Write-Host "Stopping any running components..."
Get-Process otp-relay, otp-relay-gui, otp-relay-daemon, otp-relay-bridge -ErrorAction SilentlyContinue |
  Stop-Process -Force
Start-Sleep -Milliseconds 500

Write-Host "Building (release)..."
Push-Location $root
try { cargo build --release } catch {
  Write-Host "Build failed. If 'Access is denied' on a locked exe, close the browser and retry."
  Pop-Location; exit 1
}
Pop-Location

$bridge = Join-Path $root "target\release\otp-relay-bridge.exe"
$daemon = Join-Path $root "target\release\otp-relay-daemon.exe"
$gui    = Join-Path $root "target\release\otp-relay-gui.exe"
foreach ($p in @($bridge, $daemon, $gui)) { if (-not (Test-Path $p)) { throw "missing build output: $p" } }

# --- native-messaging host -> bridge --------------------------------------
$manifestDir = Join-Path $root "host-manifest"
New-Item -ItemType Directory -Force -Path $manifestDir | Out-Null
$chromeManifest = Join-Path $manifestDir "$hostName.chrome.json"
$firefoxManifest = Join-Path $manifestDir "$hostName.firefox.json"
$bridgeJson = $bridge.Replace("\", "\\")

@"
{
  "name": "$hostName",
  "description": "Mail OTP Autofill native host",
  "path": "$bridgeJson",
  "type": "stdio",
  "allowed_origins": ["chrome-extension://$ChromeExtensionId/"]
}
"@ | Out-File -FilePath $chromeManifest -Encoding ascii

@"
{
  "name": "$hostName",
  "description": "Mail OTP Autofill native host",
  "path": "$bridgeJson",
  "type": "stdio",
  "allowed_extensions": ["$FirefoxExtensionId"]
}
"@ | Out-File -FilePath $firefoxManifest -Encoding ascii

function Register-Host($regPath, $manifestPath) {
  New-Item -Path $regPath -Force | Out-Null
  Set-ItemProperty -Path $regPath -Name "(default)" -Value $manifestPath
  Write-Host "  registered: $regPath"
}
Write-Host "Registering native-messaging host (bridge)..."
Register-Host "HKCU:\Software\Google\Chrome\NativeMessagingHosts\$hostName" $chromeManifest
Register-Host "HKCU:\Software\Microsoft\Edge\NativeMessagingHosts\$hostName" $chromeManifest
Register-Host "HKCU:\Software\Mozilla\NativeMessagingHosts\$hostName" $firefoxManifest

# --- autostart the daemon, hidden, at login -------------------------------
# A tiny VBS launcher runs the console daemon with no visible window.
$vbs = Join-Path $root "launch-daemon.vbs"
@"
Set s = CreateObject("WScript.Shell")
s.Run """$daemon""", 0, False
"@ | Out-File -FilePath $vbs -Encoding ascii

$runKey = "HKCU:\Software\Microsoft\Windows\CurrentVersion\Run"
New-Item -Path $runKey -Force | Out-Null
Set-ItemProperty -Path $runKey -Name "MailOtpAutofillDaemon" -Value ("wscript.exe `"" + $vbs + "`"")
Write-Host "  autostart registered (Run\MailOtpAutofillDaemon)"

# --- Start Menu shortcut to the GUI ---------------------------------------
$programs = [Environment]::GetFolderPath("Programs")
$lnk = Join-Path $programs "Mail OTP Autofill.lnk"
$ws = New-Object -ComObject WScript.Shell
$sc = $ws.CreateShortcut($lnk)
$sc.TargetPath = $gui
$sc.WorkingDirectory = Split-Path -Parent $gui
$sc.Description = "Manage Mail OTP Autofill accounts"
$sc.Save()
Write-Host "  Start Menu shortcut: $lnk"

# --- start the daemon now (don't wait for next login) ---------------------
Start-Process -FilePath "wscript.exe" -ArgumentList ("`"" + $vbs + "`"")
Start-Sleep -Milliseconds 800
$running = [bool](Get-Process otp-relay-daemon -ErrorAction SilentlyContinue)

Write-Host ""
Write-Host "Done. Daemon running now: $running"
Write-Host "  Add mailboxes via the GUI (Start Menu: 'Mail OTP Autofill') or:"
Write-Host "    $($root)\target\release\otp-relay.exe add --label mailcow --host mail.example.com --user you@example.com"
Write-Host "  Then set the extension's code source to 'Native app'."
Write-Host "  Debug log: $($env:TEMP)\otp_relay.log   (run the daemon with --console to watch live)"
