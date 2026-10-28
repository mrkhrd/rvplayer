@echo off
rem Configures and builds the Qt application (RelWithDebInfo, Ninja) with the Visual Studio C++ tools.
rem Qt location: QTDIR, default C:\Qt\6.10.3\msvc2022_64.
setlocal
if "%QTDIR%"=="" set QTDIR=C:\Qt\6.10.3\msvc2022_64
for /f "usebackq delims=" %%i in (`"%ProgramFiles(x86)%\Microsoft Visual Studio\Installer\vswhere.exe" -latest -products * -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath`) do set VSDIR=%%i
if "%VSDIR%"=="" (echo Visual Studio C++ tools not found & exit /b 1)
call "%VSDIR%\VC\Auxiliary\Build\vcvars64.bat" >nul || exit /b 1
if not exist "%~dp0build\build.ninja" (
    cmake -S "%~dp0." -B "%~dp0build" -G Ninja -DCMAKE_BUILD_TYPE=RelWithDebInfo -DCMAKE_PREFIX_PATH="%QTDIR%" || exit /b 1
)
cmake --build "%~dp0build" %* || exit /b 1
