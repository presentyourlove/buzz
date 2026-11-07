' Honey2::Radar 每日靜默執行包裝器
Option Explicit
Dim WshShell
Set WshShell = CreateObject("WScript.Shell")
WshShell.Run "pythonw.exe ""c:\私人\ai agent\buzz\scripts\tech_researcher.py"" --limit 3", 0, False
Set WshShell = Nothing
