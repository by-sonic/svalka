@echo off
REM Svalka dev launcher.
REM The app's manifest requests administrator rights, so a non-elevated
REM `tauri dev` cannot even spawn the built exe (CreateProcess fails with
REM error 740 "requires elevation"). This script elevates itself, then runs
REM `tauri dev` from the Tauri crate dir (where tauri.conf.json lives).

net session >nul 2>&1
if %errorlevel% neq 0 (
    echo Requesting administrator rights...
    powershell -NoProfile -Command "Start-Process -FilePath '%~f0' -Verb RunAs"
    exit /b
)

cd /d "%~dp0crates\svalka-app"
call "..\..\ui\node_modules\.bin\tauri.cmd" dev
pause
