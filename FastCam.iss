[Setup]
AppName=FastCam
AppVersion=1.0
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
PrivilegesRequired=lowest
PrivilegesRequiredOverridesAllowed=dialog
DisableProgramGroupPage=yes

[Tasks]
Name: "desktopicon"; Description: "Create a desktop shortcut"

[Files]
Source: "target\release\fastcam.exe"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{group}\FastCam"; Filename: "{app}\fastcam.exe"
Name: "{autodesktop}\FastCam"; Filename: "{app}\fastcam.exe"; Tasks: desktopicon

[Run]
Filename: "{app}\fastcam.exe"; Description: "Launch FastCam"; Flags: nowait postinstall skipifsilent
