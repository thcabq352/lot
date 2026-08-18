; Lot Windows installer. Compiled from dist/lot-windows after pack-windows.ps1.
; Ships lot.exe + lot-ui.exe only. Never Ollama, Comfy, Resolve, Blockout, or Wasserman.

Unicode true
Name "Lot"
OutFile "lot-setup.exe"
InstallDir "$LOCALAPPDATA\Lot"
RequestExecutionLevel user
SetCompressor /SOLID lzma

Page directory
Page instfiles
UninstPage uninstConfirm
UninstPage instfiles

Section "Lot"
  SetOutPath "$INSTDIR"
  File "lot.exe"
  File "lot-ui.exe"
  File "README.txt"
  File "install-shortcuts.ps1"
  CreateDirectory "$SMPROGRAMS\Lot"
  CreateShortCut "$SMPROGRAMS\Lot\Lot.lnk" "$INSTDIR\lot-ui.exe" "" "$INSTDIR\lot-ui.exe" 0
  WriteUninstaller "$INSTDIR\Uninstall.exe"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\Lot" "DisplayName" "Lot"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\Lot" "UninstallString" "$INSTDIR\Uninstall.exe"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\Lot" "DisplayIcon" "$INSTDIR\lot-ui.exe"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\Lot" "Publisher" "Lot"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\Lot" "InstallLocation" "$INSTDIR"
SectionEnd

Section "un.Uninstall"
  Delete "$INSTDIR\lot.exe"
  Delete "$INSTDIR\lot-ui.exe"
  Delete "$INSTDIR\README.txt"
  Delete "$INSTDIR\install-shortcuts.ps1"
  Delete "$INSTDIR\Uninstall.exe"
  RMDir "$INSTDIR"
  Delete "$SMPROGRAMS\Lot\Lot.lnk"
  RMDir "$SMPROGRAMS\Lot"
  DeleteRegKey HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\Lot"
SectionEnd
