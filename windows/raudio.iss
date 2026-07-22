; Inno Setup script for Raudio.
;
; Prerequisites (in the MSYS2 MinGW64 shell, from the project root):
;   cargo build --release --no-default-features   # pure GTK4, no libadwaita
;   ./windows/bundle.sh          # produces dist/raudio/
;
; Then compile this installer with Inno Setup:
;   "C:\Program Files (x86)\Inno Setup 6\ISCC.exe" windows\raudio.iss
;
; The result is dist\raudio-setup.exe.

#define AppName    "Raudio"
#define AppVersion "0.1.0"
#define AppExe     "raudio.exe"
#define AppPublisher "Raudio"

[Setup]
AppId={{E93DE611-6D9B-45FA-9D17-D9AB6D840003}
AppName={#AppName}
AppVersion={#AppVersion}
AppPublisher={#AppPublisher}
DefaultDirName={autopf}\{#AppName}
DefaultGroupName={#AppName}
UninstallDisplayIcon={app}\{#AppExe}
OutputDir=..\dist
OutputBaseFilename=raudio-setup
SetupIconFile=..\assets\icon.ico
Compression=lzma2
SolidCompression=yes
ArchitecturesInstallIn64BitMode=x64
WizardStyle=modern
DisableProgramGroupPage=yes

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"; Flags: unchecked

[Files]
; The whole bundled folder (exe + DLLs + GStreamer plugins + loaders + schemas).
Source: "..\dist\raudio\*"; DestDir: "{app}"; Flags: recursesubdirs createallsubdirs ignoreversion

[Icons]
Name: "{group}\{#AppName}";           Filename: "{app}\{#AppExe}"
Name: "{group}\Uninstall {#AppName}"; Filename: "{uninstallexe}"
Name: "{autodesktop}\{#AppName}";     Filename: "{app}\{#AppExe}"; Tasks: desktopicon

[Run]
Filename: "{app}\{#AppExe}"; Description: "{cm:LaunchProgram,{#AppName}}"; Flags: nowait postinstall skipifsilent
