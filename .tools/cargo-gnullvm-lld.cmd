@echo off
set "PATH=C:\Users\sakus\.cargo\bin;C:\Users\sakus\.rustup\toolchains\stable-x86_64-pc-windows-gnullvm\lib\rustlib\x86_64-pc-windows-gnullvm\bin;%PATH%"
set "RUSTFLAGS=-C linker=rust-lld"
C:\Users\sakus\.cargo\bin\cargo.exe +stable-x86_64-pc-windows-gnullvm %*

