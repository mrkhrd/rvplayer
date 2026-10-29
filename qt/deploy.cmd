@echo off
rem Builds the application and collects a portable copy that runs without Qt or Visual Studio
rem installed: dist\RVPlayer (rvplayer.exe with the map engine linked in, the Qt DLLs and the
rem plugins it uses, the MSVC runtime next to the exe) and dist\RVPlayer-win64.zip.
rem Qt location: QTDIR, default C:\Qt\6.10.3\msvc2022_64.
setlocal
if "%QTDIR%"=="" set QTDIR=C:\Qt\6.10.3\msvc2022_64
set DIST=%~dp0..\dist
set OUT=%DIST%\RVPlayer

call "%~dp0build.cmd" --target rvplayer || exit /b 1

rem The runtime DLLs come from the Visual Studio redistributable folder.
for /f "usebackq delims=" %%i in (`"%ProgramFiles(x86)%\Microsoft Visual Studio\Installer\vswhere.exe" -latest -products * -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath`) do set VSDIR=%%i
if "%VSDIR%"=="" (echo Visual Studio C++ tools not found & exit /b 1)
call "%VSDIR%\VC\Auxiliary\Build\vcvars64.bat" >nul || exit /b 1
set CRT=
for /d %%d in ("%VCToolsRedistDir%x64\Microsoft.VC*.CRT") do set CRT=%%d
if "%CRT%"=="" (echo MSVC runtime not found in %VCToolsRedistDir% & exit /b 1)

if exist "%OUT%" rmdir /s /q "%OUT%"
mkdir "%OUT%" || exit /b 1
copy /y "%~dp0build\rvplayer.exe" "%OUT%\" >nul || exit /b 1
"%QTDIR%\bin\windeployqt.exe" --release --no-compiler-runtime --no-system-d3d-compiler --no-system-dxc-compiler ^
    --no-opengl-sw --translations en,ru ^
    --exclude-plugins qtiff,qicns,qgif,qtga,qwbmp,qsvg,qsvgicon,qtuiotouchplugin "%OUT%\rvplayer.exe" || exit /b 1
copy /y "%CRT%\*.dll" "%OUT%\" >nul || exit /b 1

if exist "%DIST%\RVPlayer-win64.zip" del "%DIST%\RVPlayer-win64.zip"
powershell -NoProfile -Command "Compress-Archive -Path '%OUT%' -DestinationPath '%DIST%\RVPlayer-win64.zip'" || exit /b 1
echo Portable copy: %OUT%
echo Archive:       %DIST%\RVPlayer-win64.zip
