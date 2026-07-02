; Inno Setup script for Mail OTP Autofill (per-user, no admin required).
; Build the binaries first (cargo build --release in ../native-app), then compile
; this with ISCC.exe to produce Output\MailOtpAutofill-Setup.exe.

#define MyAppName "Mail OTP Autofill"
#define MyAppVersion "0.3.0"
#define ChromeExtId "bleebkmhbndbppdebamiidfilopenokn"
#define FirefoxExtId "mail-otp-autofill@mibs"
#define SrcBin "..\native-app\target\release"

[Setup]
AppId={{7B2E5F3A-9C1D-4E8B-A6F2-1D3C5E7A9B04}
AppName={#MyAppName}
AppVersion={#MyAppVersion}
AppPublisher=mibs
DefaultDirName={localappdata}\Programs\Mail OTP Autofill
DisableProgramGroupPage=yes
PrivilegesRequired=lowest
OutputDir=Output
OutputBaseFilename=MailOtpAutofill-Setup
SetupIconFile=app.ico
UninstallDisplayIcon={app}\app.ico
Compression=lzma2
SolidCompression=yes
WizardStyle=modern

[Tasks]
Name: "desktopicon"; Description: "Create a desktop shortcut"; Flags: unchecked

[Files]
Source: "{#SrcBin}\otp-relay.exe";        DestDir: "{app}"; Flags: ignoreversion
Source: "{#SrcBin}\otp-relay-bridge.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#SrcBin}\otp-relay-daemon.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#SrcBin}\otp-relay-gui.exe";    DestDir: "{app}"; Flags: ignoreversion
Source: "app.ico";                        DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{group}\Mail OTP Autofill"; Filename: "{app}\otp-relay-gui.exe"; IconFilename: "{app}\app.ico"
Name: "{userdesktop}\Mail OTP Autofill"; Filename: "{app}\otp-relay-gui.exe"; IconFilename: "{app}\app.ico"; Tasks: desktopicon

[Run]
Filename: "{app}\otp-relay-gui.exe"; Description: "Open Mail OTP Autofill to add accounts"; Flags: nowait postinstall skipifsilent

[Code]
function JsonEsc(P: String): String;
begin
  Result := P;
  StringChangeEx(Result, '\', '\\', True);
end;

procedure WriteHostFiles();
var
  App, Bridge, Daemon, ChromeM, FirefoxM, Vbs, ChromeJson, FirefoxJson: String;
begin
  App := ExpandConstant('{app}');
  Bridge := JsonEsc(App + '\otp-relay-bridge.exe');
  Daemon := App + '\otp-relay-daemon.exe';
  ChromeM := App + '\com.mibs.otp_relay.chrome.json';
  FirefoxM := App + '\com.mibs.otp_relay.firefox.json';

  ChromeJson :=
    '{' + #13#10 +
    '  "name": "com.mibs.otp_relay",' + #13#10 +
    '  "description": "Mail OTP Autofill native host",' + #13#10 +
    '  "path": "' + Bridge + '",' + #13#10 +
    '  "type": "stdio",' + #13#10 +
    '  "allowed_origins": ["chrome-extension://{#ChromeExtId}/"]' + #13#10 +
    '}' + #13#10;

  FirefoxJson :=
    '{' + #13#10 +
    '  "name": "com.mibs.otp_relay",' + #13#10 +
    '  "description": "Mail OTP Autofill native host",' + #13#10 +
    '  "path": "' + Bridge + '",' + #13#10 +
    '  "type": "stdio",' + #13#10 +
    '  "allowed_extensions": ["{#FirefoxExtId}"]' + #13#10 +
    '}' + #13#10;

  SaveStringToFile(ChromeM, ChromeJson, False);
  SaveStringToFile(FirefoxM, FirefoxJson, False);

  { register native-messaging host for Chrome, Edge, Firefox }
  RegWriteStringValue(HKCU, 'Software\Google\Chrome\NativeMessagingHosts\com.mibs.otp_relay', '', ChromeM);
  RegWriteStringValue(HKCU, 'Software\Microsoft\Edge\NativeMessagingHosts\com.mibs.otp_relay', '', ChromeM);
  RegWriteStringValue(HKCU, 'Software\Mozilla\NativeMessagingHosts\com.mibs.otp_relay', '', FirefoxM);

  { hidden autostart launcher for the daemon }
  Vbs := App + '\launch-daemon.vbs';
  SaveStringToFile(Vbs,
    'Set s = CreateObject("WScript.Shell")' + #13#10 +
    's.Run """' + Daemon + '""", 0, False' + #13#10, False);
  RegWriteStringValue(HKCU, 'Software\Microsoft\Windows\CurrentVersion\Run',
    'MailOtpAutofillDaemon', 'wscript.exe "' + Vbs + '"');
end;

function PrepareToInstall(var NeedsRestart: Boolean): String;
var
  Rc: Integer;
begin
  { free locked exes before copying }
  Exec('taskkill.exe', '/F /IM otp-relay-daemon.exe /IM otp-relay-bridge.exe /IM otp-relay-gui.exe /IM otp-relay.exe',
    '', SW_HIDE, ewWaitUntilTerminated, Rc);
  Result := '';
end;

procedure CurStepChanged(CurStep: TSetupStep);
var
  Rc: Integer;
begin
  if CurStep = ssPostInstall then
  begin
    WriteHostFiles();
    { start the daemon now (hidden), so it works before the next login }
    Exec('wscript.exe', '"' + ExpandConstant('{app}\launch-daemon.vbs') + '"',
      '', SW_HIDE, ewNoWait, Rc);
  end;
end;

procedure CurUninstallStepChanged(CurUninstallStep: TUninstallStep);
var
  Rc: Integer;
begin
  if CurUninstallStep = usUninstall then
  begin
    Exec('taskkill.exe', '/F /IM otp-relay-daemon.exe /IM otp-relay-bridge.exe /IM otp-relay-gui.exe /IM otp-relay.exe',
      '', SW_HIDE, ewWaitUntilTerminated, Rc);
    RegDeleteKeyIncludingSubkeys(HKCU, 'Software\Google\Chrome\NativeMessagingHosts\com.mibs.otp_relay');
    RegDeleteKeyIncludingSubkeys(HKCU, 'Software\Microsoft\Edge\NativeMessagingHosts\com.mibs.otp_relay');
    RegDeleteKeyIncludingSubkeys(HKCU, 'Software\Mozilla\NativeMessagingHosts\com.mibs.otp_relay');
    RegDeleteValue(HKCU, 'Software\Microsoft\Windows\CurrentVersion\Run', 'MailOtpAutofillDaemon');
  end;
end;
