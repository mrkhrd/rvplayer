@echo off
rem Builds one self-contained program file, dist\RVPlayer.exe, packed with UPX: Qt, MapLibre, the
rem Rust core and the MSVC runtime are all linked in. The first run builds a static Qt (the base
rem module) into deps\qt-static and MapLibre against it into deps\maplibre-static; later runs
rem reuse them. Downloads are pinned and checked against their published checksums.
rem   single.cmd qt     only the static Qt; single.cmd deps: the static Qt and MapLibre
rem                     (stages cached on their own, see .github/workflows)
setlocal
set QTMINOR=6.10
set QTVER=6.10.3
set QTMD5=53f04564a696246c3e115028affb6bea
set UPXVER=5.3.0
set UPXSHA=d127996c76615e431be810e9c8d2910dbe3dd400600c5c0c50276fc5a17a9705
set DEPS=%~dp0..\deps
set DIST=%~dp0..\dist
set QTSRC=%DEPS%\qt-src\qtbase-everywhere-src-%QTVER%
set QTSTATIC=%DEPS%\qt-static
set UPXEXE=%DEPS%\upx\upx-%UPXVER%-win64\upx.exe
rem Windows' own tar: a GNU tar earlier on the PATH (Git's) takes "D:\..." for a remote host.
set TAR=%SystemRoot%\System32\tar.exe
rem Downloads give up instead of hanging, and retry a failed connection.
set CURL=curl -fL --retry 3 --connect-timeout 30 --max-time 1800

for /f "usebackq delims=" %%i in (`"%ProgramFiles(x86)%\Microsoft Visual Studio\Installer\vswhere.exe" -latest -products * -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath`) do set VSDIR=%%i
if "%VSDIR%"=="" (echo Visual Studio C++ tools not found & exit /b 1)
call "%VSDIR%\VC\Auxiliary\Build\vcvars64.bat" >nul || exit /b 1

rem 1. Static Qt: the base module with the static MSVC runtime; no SQL, D-Bus or ICU.
if exist "%QTSTATIC%\lib\cmake\Qt6\Qt6Config.cmake" goto qtdone
if exist "%QTSRC%\configure.bat" goto qtsource
echo == Downloading the Qt %QTVER% base module source
mkdir "%DEPS%\qt-src" 2>nul
%CURL% -o "%DEPS%\qt-src\qtbase.tar.xz" https://download.qt.io/official_releases/qt/%QTMINOR%/%QTVER%/submodules/qtbase-everywhere-src-%QTVER%.tar.xz || exit /b 1
call :check "%DEPS%\qt-src\qtbase.tar.xz" MD5 %QTMD5% || exit /b 1
echo == Unpacking it
"%TAR%" -xf "%DEPS%\qt-src\qtbase.tar.xz" -C "%DEPS%\qt-src" || exit /b 1
del "%DEPS%\qt-src\qtbase.tar.xz"
:qtsource
echo == Configuring the static Qt
mkdir "%DEPS%\qt-static-build" 2>nul
pushd "%DEPS%\qt-static-build"
call "%QTSRC%\configure.bat" -prefix "%QTSTATIC%" -static -static-runtime -release -nomake examples ^
    -nomake tests -no-dbus -no-icu -opengl desktop -no-feature-sql || (popd & exit /b 1)
echo == Building the static Qt
cmake --build . --parallel || (popd & exit /b 1)
cmake --install . || (popd & exit /b 1)
popd
rmdir /s /q "%DEPS%\qt-static-build"
:qtdone
if /i "%~1"=="qt" exit /b 0

rem 2. MapLibre against it.
if not exist "%DEPS%\maplibre-static\lib\cmake\QMapLibre\QMapLibreConfig.cmake" (
    echo == Building MapLibre against the static Qt
    call "%~dp0maplibre.cmd" static || exit /b 1
    rmdir /s /q "%DEPS%\maplibre-static-build"
)

if /i "%~1"=="deps" exit /b 0

rem 3. The application (the Rust core built with the static runtime too, see RV_STATIC).
echo == Building the program
cmake -S "%~dp0." -B "%~dp0build-static" -G Ninja -DCMAKE_BUILD_TYPE=Release ^
    -DCMAKE_PREFIX_PATH="%QTSTATIC%" -DRV_STATIC=ON || exit /b 1
cmake --build "%~dp0build-static" --target rvplayer || exit /b 1

rem 4. Packed. UPX reads its options from a variable named UPX, hence UPXEXE. The exports (the
rem Rust core's C functions) stay uncompressed: packed, the program hangs or exits at start.
if exist "%UPXEXE%" goto upxdone
echo == Downloading UPX %UPXVER%
mkdir "%DEPS%\upx" 2>nul
%CURL% -o "%DEPS%\upx\upx.zip" https://github.com/upx/upx/releases/download/v%UPXVER%/upx-%UPXVER%-win64.zip || exit /b 1
call :check "%DEPS%\upx\upx.zip" SHA256 %UPXSHA% || exit /b 1
"%TAR%" -xf "%DEPS%\upx\upx.zip" -C "%DEPS%\upx" || exit /b 1
del "%DEPS%\upx\upx.zip"
:upxdone
echo == Packing
mkdir "%DIST%" 2>nul
if exist "%DIST%\RVPlayer.exe" del "%DIST%\RVPlayer.exe"
"%UPXEXE%" --best --lzma --compress-exports=0 -q -o "%DIST%\RVPlayer.exe" "%~dp0build-static\rvplayer.exe" || exit /b 1
echo Single file: %DIST%\RVPlayer.exe
exit /b 0

rem :check <file> <algorithm> <expected hash>
:check
echo == Checking %~nx1
powershell -NoProfile -Command "if ((Get-FileHash -Algorithm %2 '%~1').Hash -ne '%3') { Write-Error 'checksum mismatch: %~1'; exit 1 }"
exit /b %errorlevel%
