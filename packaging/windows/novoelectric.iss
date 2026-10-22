; Inno Setup script for the KoperCAD Windows installer. (This file keeps its
; novoelectric name; the exe is kopercad.exe.)
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
#define AppExe "kopercad.exe"
#define RepoRoot "..\.."

[Setup]
AppId={{AEB465B9-04F0-475B-AD7F-091D638587B5}
AppName={#AppName}
AppVersion={#AppVersion}
AppPublisher={#AppMaker}
AppPublisherURL=https://github.com/Herepiggypiggypiggy/kopercad
AppSupportURL=https://github.com/Herepiggypiggypiggy/kopercad/issues
AppUpdatesURL=https://github.com/Herepiggypiggypiggy/kopercad/releases
; An update over a running copy closes it instead of failing on a locked exe.
CloseApplications=yes
RestartApplications=no
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
Name: "associate"; Description: "Open .kopercad (and older .novo) project files with {#AppName}"; GroupDescription: "File types:"

[Files]
Source: "{#RepoRoot}\target\release\{#AppExe}"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#RepoRoot}\target\release\kopercad-cli.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#RepoRoot}\docs\TESTER-README.md"; DestDir: "{app}"; DestName: "README.md"; Flags: ignoreversion
Source: "{#RepoRoot}\RELEASE_NOTES.md"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#RepoRoot}\THIRD_PARTY_NOTICES.txt"; DestDir: "{app}"; Flags: ignoreversion
; LICENSE files (B13) are added here once the licence is decided.
Source: "{#RepoRoot}\LICENSE*"; DestDir: "{app}"; Flags: ignoreversion skipifsourcedoesntexist

[Icons]
Name: "{userprograms}\{#AppName}"; Filename: "{app}\{#AppExe}"
Name: "{userdesktop}\{#AppName}"; Filename: "{app}\{#AppExe}"; Tasks: desktopicon
; For a PC whose graphics driver fails: the software renderer, slower but it starts.
Name: "{userprograms}\{#AppName} (safe graphics)"; Filename: "{app}\{#AppExe}"; Parameters: "--safe-graphics"; Comment: "Start with the software renderer if the normal start fails"

[Registry]
Root: HKCU; Subkey: "Software\Classes\.kopercad"; ValueType: string; ValueName: ""; ValueData: "KoperCAD.Project"; Flags: uninsdeletevalue; Tasks: associate
Root: HKCU; Subkey: "Software\Classes\.novo"; ValueType: string; ValueName: ""; ValueData: "KoperCAD.Project"; Flags: uninsdeletevalue; Tasks: associate
; Older projects keep the .novo extension and still open from Explorer.
Root: HKCU; Subkey: "Software\Classes\KoperCAD.Project"; ValueType: string; ValueName: ""; ValueData: "KoperCAD project"; Flags: uninsdeletekey; Tasks: associate
Root: HKCU; Subkey: "Software\Classes\KoperCAD.Project\DefaultIcon"; ValueType: string; ValueName: ""; ValueData: "{app}\{#AppExe},0"; Tasks: associate
Root: HKCU; Subkey: "Software\Classes\KoperCAD.Project\shell\open\command"; ValueType: string; ValueName: ""; ValueData: """{app}\{#AppExe}"" ""%1"""; Tasks: associate

Root: HKCU; Subkey: "Software\Classes\{#OldAppName}.Project"; Flags: dontcreatekey deletekey

; Clean up what the installer wrote under the earlier name (NovoElectric), so
; an upgrade leaves one shortcut and one file type behind.
[InstallDelete]
Type: files; Name: "{userprograms}\{#OldAppName}.lnk"
Type: files; Name: "{userdesktop}\{#OldAppName}.lnk"
Type: files; Name: "{app}\novoelectric.exe"

[UninstallDelete]
Type: files; Name: "{userprograms}\{#AppName} (safe graphics).lnk"

[Run]
Filename: "{app}\{#AppExe}"; Description: "{cm:LaunchProgram,{#AppName}}"; Flags: nowait postinstall skipifsilent

[Code]
{ The program is removed; projects are never touched. Settings, logs and the
  recovery files stay unless the user asks for them to go too, so a later
  reinstall finds the same preferences. }
procedure CurUninstallStepChanged(CurUninstallStep: TUninstallStep);
var
  Local, Roaming: String;
begin
  if CurUninstallStep = usPostUninstall then
  begin
    Local := ExpandConstant('{localappdata}\KoperCAD');
    Roaming := ExpandConstant('{userappdata}\KoperCAD');
    if (DirExists(Local) or DirExists(Roaming)) and (not UninstallSilent) then
      if MsgBox('Also remove KoperCAD data from this PC?' + #13#10 + #13#10 +
                'This deletes your settings, parts catalog, macros, templates, report designs, ' +
                'logs, and the recovery copies of unsaved work.' + #13#10 +
                'Your saved project files are never deleted.' + #13#10 + #13#10 +
                'Choose No to keep everything for a later reinstall.', mbConfirmation, MB_YESNO or MB_DEFBUTTON2) = IDYES then
      begin
        DelTree(Local, True, True, True);
        DelTree(Roaming, True, True, True);
      end;
  end;
end;
