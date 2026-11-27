; Inno Setup 6 script for kabl's Windows installer (packaging/windows/README.md). It installs the
; folder stage.sh writes: VST3\ and CLAP\ into the common plugin folders, the rest into
; Program Files\kabl. Per-machine, so it needs admin rights. User data (%APPDATA%\kabl,
; %LOCALAPPDATA%\kabl) is never installed or removed here.
;
;   iscc /DAppVersion=<version> /DStageDir=<staged folder> /DOutputDir=<dir> kabl.iss
#ifndef AppVersion
  #error Pass /DAppVersion=<version>
#endif
#ifndef StageDir
  #error Pass /DStageDir=<the folder stage.sh wrote>
#endif
#ifndef OutputDir
  #define OutputDir "."
#endif

[Setup]
; Fixed for every release: Windows matches upgrades and the uninstall entry by this id.
AppId={{733C72D0-7569-4CAD-85EA-AA24647F7334}
AppName=kabl
AppVersion={#AppVersion}
AppVerName=kabl {#AppVersion}
AppPublisher=Kosta Vukićević
AppPublisherURL=https://github.com/stcksmsh/kabl
AppCopyright=Copyright (C) 2026 Kosta Vukićević
DefaultDirName={autopf}\kabl
DisableProgramGroupPage=yes
PrivilegesRequired=admin
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
LicenseFile={#StageDir}\LICENSE
OutputDir={#OutputDir}
OutputBaseFilename=kabl-{#AppVersion}-windows-x64-setup
SetupIconFile=..\..\..\crates\ui\assets\windows\kabl.ico
UninstallDisplayIcon={app}\kabl-ui.exe
UninstallDisplayName=kabl {#AppVersion}
WizardStyle=modern
Compression=lzma2
SolidCompression=yes

[InstallDelete]
; Factory content and the bundle are replaced whole, so an upgrade leaves no sound or file
; that the new version dropped.
Type: filesandordirs; Name: "{app}\share\kabl"
Type: filesandordirs; Name: "{commoncf64}\VST3\kabl.vst3"

[Files]
Source: "{#StageDir}\*"; Excludes: "\VST3,\CLAP"; DestDir: "{app}"; Flags: ignoreversion recursesubdirs createallsubdirs
Source: "{#StageDir}\VST3\kabl.vst3\*"; DestDir: "{commoncf64}\VST3\kabl.vst3"; Flags: ignoreversion recursesubdirs createallsubdirs
Source: "{#StageDir}\CLAP\kabl.clap"; DestDir: "{commoncf64}\CLAP"; Flags: ignoreversion

[Icons]
Name: "{autoprograms}\kabl"; Filename: "{app}\kabl-ui.exe"; WorkingDir: "{app}"

[Run]
Filename: "{app}\kabl-ui.exe"; Description: "Start kabl"; Flags: nowait postinstall skipifsilent
