@echo off
rem Runs the built application with the Qt DLLs on PATH. Arguments are passed through.
setlocal
if "%QTDIR%"=="" set QTDIR=C:\Qt\6.10.3\msvc2022_64
set PATH=%QTDIR%\bin;%PATH%
"%~dp0build\rvplayer.exe" %*
