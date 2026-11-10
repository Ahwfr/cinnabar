@echo off
powershell.exe -NoProfile -File "%~dp0Start-Builder.ps1"
if errorlevel 1 pause
