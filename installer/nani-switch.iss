#ifndef MyAppVersion
  #define MyAppVersion "0.1.0"
#endif
#ifndef MyAppNumericVersion
  #define MyAppNumericVersion "0.1.0"
#endif

#define MyAppName "Nani Switch"
#define MyAppExeName "nani-switch.exe"

[Setup]
AppId={{C75DEB3C-504A-4977-9CB7-A7A60CB93190}
AppName={#MyAppName}
AppVersion={#MyAppVersion}
AppPublisher=Tsuuko
VersionInfoVersion={#MyAppNumericVersion}
DefaultDirName={localappdata}\Programs\Nani Switch
DefaultGroupName=Nani Switch
DisableProgramGroupPage=yes
PrivilegesRequired=lowest
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
UninstallDisplayIcon={app}\{#MyAppExeName}
OutputDir=..\artifacts
OutputBaseFilename=nani-switch-v{#MyAppVersion}-windows-x86_64-setup
SetupIconFile=..\assets\tray.ico
Compression=lzma2
SolidCompression=yes
WizardStyle=modern
CloseApplications=force
RestartApplications=yes

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"
Name: "japanese"; MessagesFile: "compiler:Languages\Japanese.isl"

[Files]
Source: "..\target\x86_64-pc-windows-msvc\release\{#MyAppExeName}"; DestDir: "{app}"; Flags: ignoreversion restartreplace

[Icons]
Name: "{group}\{#MyAppName}"; Filename: "{app}\{#MyAppExeName}"

[Run]
Filename: "{app}\{#MyAppExeName}"; Description: "Launch {#MyAppName}"; Flags: nowait postinstall skipifsilent

[Code]
procedure CurUninstallStepChanged(CurUninstallStep: TUninstallStep);
begin
  if CurUninstallStep = usUninstall then
  begin
    RegDeleteValue(
      HKEY_CURRENT_USER,
      'Software\Microsoft\Windows\CurrentVersion\Run',
      '{#MyAppName}'
    );
    RegDeleteValue(
      HKEY_CURRENT_USER,
      'Software\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\Run',
      '{#MyAppName}'
    );
  end;
end;
