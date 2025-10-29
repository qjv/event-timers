@echo off
setlocal enabledelayedexpansion

echo ========================================
echo Building Event Timers for GW2
echo ========================================
echo.

REM Build the project
echo [1/3] Building with cargo...
cargo build --release

REM Check if build was successful
if %ERRORLEVEL% NEQ 0 (
    echo.
    echo [ERROR] Build failed! Please fix the errors above.
    pause
    exit /b 1
)

echo.
echo [2/3] Checking for DLL files...

REM Count DLL files in target/release
set DLL_COUNT=0
set DLL_NAME=

for %%F in (target\release\*.dll) do (
    set /a DLL_COUNT+=1
    set DLL_NAME=%%~nxF
    set DLL_PATH=%%F
)

REM Check DLL count
if %DLL_COUNT% EQU 0 (
    echo [ERROR] No DLL file found in target\release\
    echo Expected to find event_timers.dll
    pause
    exit /b 1
)

if %DLL_COUNT% GTR 1 (
    echo [ERROR] Multiple DLL files found in target\release\:
    echo.
    for %%F in (target\release\*.dll) do (
        echo   - %%~nxF
    )
    echo.
    echo There should only be ONE DLL file ^(event_timers.dll^)
    echo Please remove the extra DLL files and try again.
    pause
    exit /b 1
)

echo Found: %DLL_NAME%

REM Verify it's the expected DLL
if /i NOT "%DLL_NAME%"=="event_timers.dll" (
    echo [WARNING] Found '%DLL_NAME%' but expected 'event_timers.dll'
    echo Continuing anyway...
)

echo.
echo [3/3] Copying to Guild Wars 2 addons folder...

REM Create addons directory if it doesn't exist
if not exist "D:\Guild Wars 2\addons\" (
    echo Creating addons directory...
    mkdir "D:\Guild Wars 2\addons\"
)

REM Copy the DLL
copy /Y "%DLL_PATH%" "D:\Guild Wars 2\addons\" >nul

if %ERRORLEVEL% EQU 0 (
    echo.
    echo ========================================
    echo [SUCCESS] Build complete!
    echo ========================================
    echo DLL copied to: D:\Guild Wars 2\addons\%DLL_NAME%
    echo.
) else (
    echo.
    echo [ERROR] Failed to copy DLL to Guild Wars 2 folder
    echo Make sure the folder exists and you have write permissions
    pause
    exit /b 1
)

pause
