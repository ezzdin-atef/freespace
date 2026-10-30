; Hooks for the Tauri NSIS installer (bundle.windows.nsis.installerHooks in tauri.conf.json).

; Up to 1.0.0 Freespace installed per user into %LOCALAPPDATA%\Freespace. It now installs per machine into
; Program Files, so remove an old per-user copy first; otherwise the app would be installed twice. The silent
; uninstaller keeps the app's data.
!macro NSIS_HOOK_PREINSTALL
  ReadRegStr $0 HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\${PRODUCTNAME}" "UninstallString"
  ${If} $0 != ""
    ; $0 is "<dir>\uninstall.exe" with quotes: strip the leading quote and the trailing \uninstall.exe".
    StrCpy $1 $0 "" 1
    StrCpy $1 $1 -15
    ${If} ${FileExists} "$1\uninstall.exe"
      DetailPrint "Removing the previous per-user install in $1"
      ; _?= makes the uninstaller run in place, so ExecWait really waits for it.
      ExecWait '"$1\uninstall.exe" /S _?=$1'
      Delete "$1\uninstall.exe"
      RMDir "$1"
    ${EndIf}
    DeleteRegKey HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\${PRODUCTNAME}"
  ${EndIf}
!macroend
