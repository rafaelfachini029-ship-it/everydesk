; EveryDesk NSIS installer hooks (Tauri 2: bundle.windows.nsis.installerHooks).
; Placeholder — Etapa 5 Task 2 fills these in (service install/uninstall etc.).

!macro NSIS_HOOK_PREINSTALL
  ; Runs before files are copied, registry keys set and shortcuts created.
!macroend

!macro NSIS_HOOK_POSTINSTALL
  ; Runs after the installer has finished copying files and creating shortcuts.
!macroend

!macro NSIS_HOOK_PREUNINSTALL
  ; Runs before files, registry keys and shortcuts are removed.
!macroend

!macro NSIS_HOOK_POSTUNINSTALL
  ; Runs after files, registry keys and shortcuts have been removed.
!macroend
