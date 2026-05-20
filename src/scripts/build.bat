@echo off
echo === Backyard Build ===
echo.

REM Add Rust toolchain to PATH
if exist "%USERPROFILE%\.cargo\bin" set "PATH=%USERPROFILE%\.cargo\bin;%PATH%"

echo [1/2] Building frontend...
cd /d "%~dp0..\frontend"
call npm install
call npm run build
if %ERRORLEVEL% neq 0 (
    echo Frontend build failed!
    exit /b %ERRORLEVEL%
)
echo [2/2] Building Tauri desktop app...
cd /d "%~dp0..\backend"
cargo build --release
if %ERRORLEVEL% neq 0 (
    echo Tauri build failed!
    exit /b %ERRORLEVEL%
)
echo.
echo Build complete! Binary at: tauri\target\release\backyard.exe
pause
