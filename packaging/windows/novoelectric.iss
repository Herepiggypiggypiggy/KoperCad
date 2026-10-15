; Inno Setup script for the KoperCAD Windows installer. (The exe and this file
; keep their novoelectric names until the repository itself is renamed.)
;
; Build locally (after `cargo build --release -p novo-app`):
;   iscc /DAppVersion=0.1.0 packaging\windows\novoelectric.iss
; The installer is written to dist\.

#ifndef AppVersion
  #define AppVersion "0.0.0"
#endif

#define AppName "KoperCAD"
#define AppMaker "KoperVonk"
#define OldAppName "NovoElectric"
#define AppExe "novoelectric.exe"
#define RepoRoot "..\.."

[Setup]
AppId={{AEB465B9-04F0-475B-AD7F-091D638587B5}
AppName={#AppName}
AppVersion={#AppVersion}
AppPublisher={#AppMaker}
AppPublisherURL=https://github.com/Herepiggypiggypiggy/kopercad
; Strictly per-user: no admin rights are ever requested, everything goes to
; %LOCALAPPDATA% and HKCU.
DefaultDirName={localappdata}\Programs\{#AppName}
DefaultGroupName={#AppName}
DisableProgramGroupPage=yes
; A per-user install always goes to the user's own folder.
DisableDirPage=yes
PrivilegesRequired=lowest
UsedUserAreasWarning=no
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
OutputDir={#RepoRoot}\dist
OutputBaseFilename=KoperCAD-{#AppVersion}-setup
SetupIconFile={#RepoRoot}\assets\branding\icon.ico
UninstallDisplayIcon={app}\{#AppExe}
Compression=lzma2/max
SolidCompression=yes
WizardStyle=modern
ChangesAssociations=yes

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"; Flags: unchecked
Name: "associate"; Description: "Open .novo project files with {#AppName}"; GroupDescription: "File types:"

[Files]
Source: "{#RepoRoot}\target\release\{#AppExe}"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{userprograms}\{#AppName}"; Filename: "{app}\{#AppExe}"
Name: "{userdesktop}\{#AppName}"; Filename: "{app}\{#AppExe}"; Tasks: desktopicon

[Registry]
Root: HKCU; Subkey: "Software\Classes\.novo"; ValueType: string; ValueName: ""; ValueData: "KoperCAD.Project"; Flags: uninsdeletevalue; Tasks: associate
Root: HKCU; Subkey: "Software\Classes\KoperCAD.Project"; ValueType: string; ValueName: ""; ValueData: "KoperCAD project"; Flags: uninsdeletekey; Tasks: associate
Root: HKCU; Subkey: "Software\Classes\KoperCAD.Project\DefaultIcon"; ValueType: string; ValueName: ""; ValueData: "{app}\{#AppExe},0"; Tasks: associate
Root: HKCU; Subkey: "Software\Classes\KoperCAD.Project\shell\open\command"; ValueType: string; ValueName: ""; ValueData: """{app}\{#AppExe}"" ""%1"""; Tasks: associate

Root: HKCU; Subkey: "Software\Classes\{#OldAppName}.Project"; Flags: dontcreatekey deletekey

; Clean up what the installer wrote under the earlier name (NovoElectric), so
; an upgrade leaves one shortcut and one file type behind.
[InstallDelete]
Type: files; Name: "{userprograms}\{#OldAppName}.lnk"
Type: files; Name: "{userdesktop}\{#OldAppName}.lnk"

[Run]
Filename: "{app}\{#AppExe}"; Description: "{cm:LaunchProgram,{#AppName}}"; Flags: nowait postinstall skipifsilent
