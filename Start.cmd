@echo off
setlocal
where py.exe >nul 2>nul
if errorlevel 1 goto find_python
py.exe -3 -c "import sys; raise SystemExit(sys.version_info < (3,11))" >nul 2>nul
if not errorlevel 1 goto use_py
:find_python
where python.exe >nul 2>nul
if not errorlevel 1 goto use_python
echo Python 3.11 oder neuer, 64 Bit, mit Tk wird benoetigt.
echo Bitte den offiziellen Windows-Installer von https://www.python.org/downloads/windows/ installieren.
echo Danach Start.cmd erneut oeffnen. Visual Studio wird nicht benoetigt.
if "%~1"=="" pause
exit /b 1

:use_py
py.exe -3 "%~dp0launcher\start.py" %*
set "HI_EXIT=%errorlevel%"
goto finish

:use_python
python.exe "%~dp0launcher\start.py" %*
set "HI_EXIT=%errorlevel%"

:finish
if not "%HI_EXIT%"=="0" if "%~1"=="" pause
exit /b %HI_EXIT%
