@echo off
rem  No 18 - Galaxy Colonization Simulator
rem  Double-click to build and run. First build takes ~2 minutes; after that
rem  it starts in a couple of seconds.
rem
rem    run.bat            the simulator
rem    run.bat limits     headless probe of this GPU's allocation limits
rem    run.bat test       shader validation + sampling statistics

cd /d "%~dp0"

where cargo >nul 2>&1
if errorlevel 1 (
    echo.
    echo   Rust toolchain not found on PATH.
    echo   Install from https://rustup.rs then reopen this window.
    echo.
    pause
    exit /b 1
)

if /i "%~1"=="limits" (
    cargo run --release --example limits
    echo.
    pause
    exit /b 0
)

if /i "%~1"=="test" (
    cargo test --release -- --nocapture
    echo.
    pause
    exit /b 0
)

cargo run --release
if errorlevel 1 (
    echo.
    echo   Build or run failed. If it says "Access is denied" on the .exe,
    echo   an older copy is still running - close that window and retry.
    echo.
    pause
)
