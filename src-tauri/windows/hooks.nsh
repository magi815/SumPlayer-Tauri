; Silently uninstall previous version to skip the overwrite/uninstall dialog
!macro NSIS_HOOK_PREINSTALL
  ReadRegStr $R0 SHCTX "Software\Microsoft\Windows\CurrentVersion\Uninstall\${PRODUCTNAME}" "QuietUninstallString"
  ${If} $R0 != ""
    ExecWait '$R0'
  ${EndIf}
!macroend
