; ReelForge installer (Inno Setup 6).

#ifndef AppVersion
  #define AppVersion "1.0.0"
#endif

[Setup]
AppId={{C8A4E1D7-2B36-4F9A-8E05-1A6B9D3C7F52}
AppName=ReelForge
AppVersion={#AppVersion}
AppVerName=ReelForge {#AppVersion}
AppPublisher=Zheer Khalid
AppPublisherURL=https://zheer.dev
AppSupportURL=https://github.com/zheerkhalid/reelforge
AppUpdatesURL=https://github.com/zheerkhalid/reelforge/releases
DefaultDirName={autopf}\ReelForge
DefaultGroupName=ReelForge
DisableProgramGroupPage=yes
PrivilegesRequired=lowest
PrivilegesRequiredOverridesAllowed=dialog
ArchitecturesAllowed=x64compatible 
ArchitecturesInstallIn64BitMode=x64compatible 
OutputDir=..\dist
OutputBaseFilename=ReelForge-Setup
SetupIconFile=..\target\reelforge.ico
UninstallDisplayIcon={app}\ReelForge.exe
Compression=lzma2/ultra64
SolidCompression=yes
WizardStyle=modern
CloseApplications=yes
RestartApplications=no
LicenseFile=..\LICENSE
VersionInfoVersion={#AppVersion}
VersionInfoCompany=Zheer Khalid (zheer.dev)
VersionInfoDescription=ReelForge Setup

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"; 

[Files]
Source: "..\target\release\ReelForge.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\LICENSE"; DestDir: "{app}"; DestName: "LICENSE.txt"; Flags: ignoreversion

[Icons]
Name: "{group}\ReelForge"; Filename: "{app}\ReelForge.exe"
Name: "{group}\Uninstall ReelForge"; Filename: "{uninstallexe}"
Name: "{autodesktop}\ReelForge"; Filename: "{app}\ReelForge.exe"; Tasks: desktopicon


[Run]
Filename: "{app}\ReelForge.exe"; Description: "{cm:LaunchProgram,ReelForge}"; Flags: nowait postinstall skipifsilent shellexec

[UninstallRun]
Filename: "{cmd}"; Parameters: "/C taskkill /IM ReelForge.exe /F"; Flags: runhidden; RunOnceId: "KillReelForge"
