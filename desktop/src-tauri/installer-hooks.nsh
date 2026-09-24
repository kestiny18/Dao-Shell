; Keep application binaries separate from %LOCALAPPDATA%\Dao-Shell user data.
; GUI initialization occurs after Tauri restores a previous/custom install path.
!define MUI_CUSTOMFUNCTION_GUIINIT DaoShellInstallDirectory

Function DaoShellInstallDirectory
  ${If} $INSTDIR == "$LOCALAPPDATA\Dao-Shell"
    StrCpy $INSTDIR "$LOCALAPPDATA\Programs\Dao-Shell"
  ${EndIf}
FunctionEnd

!macro NSIS_HOOK_PREINSTALL
  ; Silent installation has no GUI initialization callback.
  Call DaoShellInstallDirectory
  SetOutPath $INSTDIR
!macroend
