; Rocket NSIS install hooks.
;
; POSTINSTALL   — register the privileged helper service (RocketVpnSvc).
; PREUNINSTALL  — stop & remove the service and its runtime data.
; POSTUNINSTALL — remove the user's config, the autostart entry and any
;                 leftover data directory.

!macro NSIS_HOOK_POSTINSTALL
  DetailPrint "Rocket VPN helper service..."
  nsExec::ExecToLog '"$INSTDIR\rocket-svc.exe" install'
  Pop $0
!macroend

!macro NSIS_HOOK_PREUNINSTALL
  DetailPrint "Removing Rocket VPN helper service..."
  ; try the freshly-shipped exe first, then whatever the service was aimed at
  nsExec::ExecToLog '"$INSTDIR\rocket-svc.exe" uninstall'
  Pop $0
  nsExec::ExecToLog 'sc.exe stop RocketVpnSvc'
  Pop $0
  nsExec::ExecToLog 'sc.exe delete RocketVpnSvc'
  Pop $0

  ; runtime data written by the service (config, cache, logs, staged cores)
  SetShellVarContext all
  RMDir /r "$APPDATA\Rocket"
  SetShellVarContext current
!macroend

!macro NSIS_HOOK_POSTUNINSTALL
  DetailPrint "Cleaning up Rocket data..."
  ; per-user config (servers, subscriptions, settings)
  RMDir /r "$APPDATA\Rocket"
  ; autostart entry added by tauri-plugin-autostart
  DeleteRegValue HKCU "Software\Microsoft\Windows\CurrentVersion\Run" "Rocket"
  DeleteRegValue HKCU "Software\Microsoft\Windows\CurrentVersion\Run" "com.rocket.win"
!macroend
