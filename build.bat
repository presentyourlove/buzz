@echo off
set PATH=C:\Users\user\.cargo\bin;C:\Users\user\.local\mingw64\mingw64\bin;%PATH%
set LIBRARY_PATH=C:\Users\user\.local\mingw64\mingw64\lib
C:\Users\user\.cargo\bin\cargo.exe build --release -p buzz-agent 2>&1
