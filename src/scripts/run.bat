@echo off
echo === Backyard Build ^& Run ===
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
echo [2/2] Building and running Tauri app...
cd /d "%~dp0..\backend"
cargo run