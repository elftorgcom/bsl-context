@echo off
rem run.bat — скрытый запуск bsl-context-rs из Task Scheduler / nssm.
rem Канон: каталог логов создаётся ДО запуска exe (иначе cmd.exe не сможет
rem открыть файл для перенаправления и упадёт с rc=1 без причины — поймано
rem на 1c-ops 2026-05-05).

if not exist "C:\bsl-context-rs\logs" mkdir "C:\bsl-context-rs\logs"

set BSL_CONTEXT_CONFIG=C:\tools\bsl-context-rs\configs\config.toml
set RUST_BACKTRACE=1

"C:\tools\bsl-context-rs\bin\bsl-context-rs.exe" --config %BSL_CONTEXT_CONFIG% >> "C:\bsl-context-rs\logs\stdout.log" 2>> "C:\bsl-context-rs\logs\stderr.log"
