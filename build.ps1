$env:Path += ';C:\Users\user\.cargo\bin;C:\Users\user\.local\mingw64\w64devkit\bin'
$env:LIBRARY_PATH = 'C:\Users\user\.local\mingw64\w64devkit\lib'
& 'C:\Users\user\.cargo\bin\cargo.exe' build --release -p buzz-agent 2>&1
