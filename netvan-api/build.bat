@echo off
call "D:\VSBuildTools\Common7\Tools\VsDevCmd.bat" -arch=amd64
cd /d C:\Users\a.dashti\GitHub\alamut-api
echo LIB=%LIB%
cargo build -p alamut-api
