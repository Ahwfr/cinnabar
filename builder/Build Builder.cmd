@echo off
powershell.exe -NoProfile -ExecutionPolicy RemoteSigned -File "%~dp0Build-Builder.ps1"
pause
