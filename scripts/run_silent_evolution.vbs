' Honey2 每日進化建議書 靜默執行包裝器
Option Explicit
Dim WshShell
Set WshShell = CreateObject("WScript.Shell")
WshShell.Run "pythonw.exe ""c:\私人\ai agent\buzz\scripts\daily_evolution_dm.py""", 0, False
Set WshShell = Nothing
