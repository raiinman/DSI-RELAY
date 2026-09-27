@echo off
setlocal
powershell.exe -NoProfile -ExecutionPolicy Bypass -File "%~dp0Uninstall-RELAY.ps1"
if errorlevel 1 (
  echo.
  echo Removal did not complete. Review the message above.
  pause
  exit /b 1
)
echo.
echo Press any key to close this window.
pause >nul
