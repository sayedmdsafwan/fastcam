[Setup]
AppId={{8B1A2C3D-4E5F-6A7B-8C9D-0E1F2A3B4C5D}}
AppName=FastCam
AppVersion=1.1.0
AppVerName=FastCam 1.1.0
AppPublisher=Md. Safwan
AppPublisherURL=https://github.com/sayedmdsafwan/fastcam
AppSupportURL=https://github.com/sayedmdsafwan/fastcam/issues
DefaultDirName={autopf}\FastCam
DefaultGroupName=FastCam
OutputDir=installer
OutputBaseFilename=FastCam_Setup
SetupIconFile=icon.ico
UninstallDisplayIcon={app}\fastcam.exe
Compression=lzma2
SolidCompression=yes
WizardStyle=modern
PrivilegesRequired=lowest
PrivilegesRequiredOverridesAllowed=dialog
DisableProgramGroupPage=yes
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
MinVersion=10.0

[Tasks]
Name: "desktopicon"; Description: "Create a desktop shortcut"; Flags: unchecked

[Files]
Source: "target\release\fastcam.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "README.md"; DestDir: "{app}"; Flags: ignoreversion
Source: "LICENSE"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{group}\FastCam"; Filename: "{app}\fastcam.exe"
Name: "{group}\Uninstall FastCam"; Filename: "{uninstallexe}"
Name: "{autodesktop}\FastCam"; Filename: "{app}\fastcam.exe"; Tasks: desktopicon

[Run]
Filename: "{app}\fastcam.exe"; Description: "Launch FastCam"; Flags: nowait postinstall skipifsilent
