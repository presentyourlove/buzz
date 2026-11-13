$env:PATH = 'C:\Users\user\.cargo\bin;C:\Users\user\.local\mingw64\w64devkit\bin;' + $env:PATH
$env:LIBRARY_PATH = 'C:\Users\user\.local\mingw64\w64devkit\lib'
$env:CARGO_TARGET_DIR = 'C:\buzz-target'
Set-Location $PSScriptRoot
cargo build --release -p buzz-agent --bin buzz-agent 2>&1
