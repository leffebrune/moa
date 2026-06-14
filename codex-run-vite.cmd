@echo off
cd /d "%~dp0"
npm.cmd run dev -- --host 127.0.0.1 > vite-dev.log 2> vite-dev.err.log
