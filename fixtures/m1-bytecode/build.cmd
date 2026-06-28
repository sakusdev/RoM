@echo off
setlocal
set ROOT=%~dp0
set OUT=%ROOT%out
if exist "%OUT%" rmdir /s /q "%OUT%"
mkdir "%OUT%\classes"
javac -g -d "%OUT%\classes" "%ROOT%src\m1\BytecodeFeatures.java"
jar --create --file "%OUT%\m1-bytecode.jar" -C "%OUT%\classes" .
