@echo off
rem Fetches and builds MapLibre Native for Qt, the map engine (vector and raster maps), as static
rem libraries linked into rvplayer.exe. Run once before build.cmd, and again after changing TAG.
rem   maplibre.cmd         against the installed Qt (QTDIR, default C:\Qt\6.10.3\msvc2022_64),
rem                        into deps\maplibre
rem   maplibre.cmd static  against the static Qt of single.cmd (deps\qt-static) with the static
rem                        MSVC runtime, into deps\maplibre-static
setlocal
set TAG=v3.0.0
if "%QTDIR%"=="" set QTDIR=C:\Qt\6.10.3\msvc2022_64
set DEPS=%~dp0..\deps
set SRC=%DEPS%\maplibre-native-qt
set BUILD=%DEPS%\maplibre-build
set PREFIX=%DEPS%\maplibre
set RUNTIME=MultiThreadedDLL
if /i "%~1"=="static" (
    set QTDIR=%DEPS%\qt-static
    set BUILD=%DEPS%\maplibre-static-build
    set PREFIX=%DEPS%\maplibre-static
    set RUNTIME=MultiThreaded
)

for /f "usebackq delims=" %%i in (`"%ProgramFiles(x86)%\Microsoft Visual Studio\Installer\vswhere.exe" -latest -products * -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath`) do set VSDIR=%%i
if "%VSDIR%"=="" (echo Visual Studio C++ tools not found & exit /b 1)
call "%VSDIR%\VC\Auxiliary\Build\vcvars64.bat" >nul || exit /b 1

if not exist "%SRC%\CMakeLists.txt" (
    git clone --depth 1 --branch %TAG% --recurse-submodules --shallow-submodules ^
        https://github.com/maplibre/maplibre-native-qt.git "%SRC%" || exit /b 1
)
rem No Qt Location plugin; SQLite (the tile cache) built in instead of Qt Sql. C4834: newer Qt
rem marks QFile::open [[nodiscard]], which the library's /WX turns into an error.
rem QT_MAPLIBRE_STATIC: the headers neither export nor import (users define it too).
cmake -S "%SRC%" -B "%BUILD%" -G Ninja -DCMAKE_BUILD_TYPE=Release -DCMAKE_PREFIX_PATH="%QTDIR%" ^
    -DCMAKE_CXX_FLAGS="/DWIN32 /D_WINDOWS /EHsc /wd4834 /DQT_MAPLIBRE_STATIC" ^
    -DCMAKE_MSVC_RUNTIME_LIBRARY=%RUNTIME% ^
    -DCMAKE_INSTALL_PREFIX="%PREFIX%" -DMLN_QT_STATIC=ON -DMLN_QT_WITH_WIDGETS=ON ^
    -DMLN_QT_WITH_LOCATION=OFF -DMLN_QT_WITH_INTERNAL_SQLITE=ON || exit /b 1
cmake --build "%BUILD%" || exit /b 1
cmake --install "%BUILD%" || exit /b 1
echo MapLibre installed in %PREFIX%
