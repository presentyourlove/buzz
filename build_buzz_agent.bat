@echo off
set PATH=C:\Users\user\.cargo\bin;C:\Users\user\.local\mingw64\w64devkit\bin;%PATH%
set LIBRARY_PATH=C:\Users\user\.local\mingw64\w64devkit\lib
cd /d C:\私人\ai agent\buzz
cargo build --release -p buzz-agent 2>&1
