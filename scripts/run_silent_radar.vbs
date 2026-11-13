' Honey2::Radar 每日靜默執行包裝器 (附帶日誌紀錄與自動容錯)
Option Explicit
Dim WshShell
Set WshShell = CreateObject("WScript.Shell")
WshShell.Run "cmd.exe /c """"""C:\Users\user\AppData\Local\Python\pythoncore-3.15-64\python.exe"""" """"c:\私人\ai agent\buzz\scripts\tech_researcher.py"""" --limit 3 >> """"C:\Users\user\.buzz\PLANS\REPORTS\radar_daily.log"""" 2>&1""", 0, False
Set WshShell = Nothing
