!macro NSIS_HOOK_PREUNINSTALL
  ${If} $UpdateMode <> 1
  ${AndIf} $PassiveMode <> 1
  ${AndIfNot} ${Silent}
    MessageBox MB_OK|MB_ICONINFORMATION "Your Meetings are kept in $DOCUMENTS\Meeting Minutes. Uninstalling never removes them."
  ${EndIf}
!macroend
