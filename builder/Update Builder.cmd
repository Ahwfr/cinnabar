@echo off
powershell.exe -NoProfile -ExecutionPolicy RemoteSigned -File "%~dp0Update-Builder.ps1" %*
pause
