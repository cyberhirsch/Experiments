@echo off
rem  No 19 - Splat Tracer
rem
rem    run.bat                     open the tracer, then drop a .ply on the window
rem    drag a .ply onto run.bat    open it directly
rem    run.bat check scene.ply     safety check: GPU proof, then ramped runs on the scan
rem    run.bat compare scene.ply   tracer vs the 3DGS rasteriser: PSNR, timings, PNGs
rem    run.bat test                GPU-vs-CPU proof, reference cross-checks, shaders
rem
rem  First build takes a couple of minutes. Needs an NVIDIA/AMD GPU with
rem  hardware ray tracing and a current Vulkan driver.

cd /d "%~dp0"

where cargo >nul 2>&1
if errorlevel 1 (
    echo.
    echo   Rust toolchain not found on PATH. Install from https://rustup.rs
    echo.
    pause
    exit /b 1
)

if /i "%~1"=="check" goto check

if /i "%~1"=="test" (
    cargo test --release -- --nocapture --test-threads=1
    echo.
    pause
    exit /b 0
)

if /i "%~1"=="compare" (
    if "%~2"=="" (
        echo   usage: run.bat compare path\to\point_cloud.ply [--width 960] [--views 5]
        pause
        exit /b 2
    )
    cargo run --release --example compare -- "%~2" %3 %4 %5 %6 %7 %8
    echo.
    pause
    exit /b 0
)

if "%~1"=="" (
    cargo run --release
) else (
    cargo run --release -- "%~1"
)
if errorlevel 1 (
    echo.
    echo   Exited with an error. "Access is denied" on the .exe means an older
    echo   copy is still running - close it and retry.
    echo.
    pause
)
exit /b 0


rem ---------------------------------------------------------------------------
rem  Safety check. Three steps, each only if the one before passed:
rem    1  the GPU proof on 600 synthetic splats - under two seconds
rem    2  the ramp with the BOX proxy on the scan: grows from 50 k splats at
rem       160x90 up to the full scan at 640x360, timing every GPU job, and
rem       stops itself before any step measured or predicted over 100 ms per
rem       tile or 500 ms of BVH build
rem    3  the same ramp with the ORIENTED proxy - asks first
rem  Everything goes to check_log.txt beside this file.
rem ---------------------------------------------------------------------------
:check
if "%~2"=="" (
    echo   usage: run.bat check path\to\scene.ply
    pause
    exit /b 2
)
rem  (outside a parenthesised block: a path containing ")" would end it early)
if not exist "%~2" goto check_missing
set "LOG=%~dp0check_log.txt"
echo No 19 safety check  %date% %time%  on %~2 > "%LOG%"
echo.
echo   Save open work first - the two earlier crashes needed a hard reset.
echo   Each step stops itself if a GPU job measures or is predicted over its limit.
echo.

echo [0/3] building, no GPU use ...
cargo build --release --tests --examples -q >> "%LOG%" 2>&1
if errorlevel 1 (
    echo       build failed
    goto check_done
)

echo [1/3] GPU proof: 600 synthetic splats at 128x96 ...
echo. >> "%LOG%"
echo ===== step 1: GPU proof ===== >> "%LOG%"
cargo test --release -q --test gpu -- --nocapture --test-threads=1 >> "%LOG%" 2>&1
if errorlevel 1 (
    echo       FAILED
    goto check_done
)
echo       passed

echo [2/3] ramp, box proxy, on %~nx2 ...
echo. >> "%LOG%"
echo ===== step 2: ramp, box proxy ===== >> "%LOG%"
cargo run --release -q --example diagnose -- "%~2" --gpu-only >> "%LOG%" 2>&1
if errorlevel 1 (
    echo       STOPPED or failed
    goto check_done
)
echo       passed

echo.
echo   Step 3 runs the ORIENTED proxy - the mode whose first full-scale run
echo   crashed this PC. It is guarded by the ramp alone: the per-ray cap cannot
echo   see its work. Close this window now to skip it, or
pause
echo [3/3] ramp, oriented proxy ...
echo. >> "%LOG%"
echo ===== step 3: ramp, oriented proxy ===== >> "%LOG%"
cargo run --release -q --example diagnose -- "%~2" --gpu-only --oriented >> "%LOG%" 2>&1
if errorlevel 1 (
    echo       STOPPED or failed
    goto check_done
)
echo       passed

:check_done
echo.
echo   Log: %LOG%
echo.
pause
exit /b 0

:check_missing
echo   not found: %~2
pause
exit /b 2
