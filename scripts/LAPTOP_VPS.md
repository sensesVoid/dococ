# Laptop as VPS (Development/Paper Trading)

## Overview
Use your Windows laptop as the Doc Oc daemon host with ngrok for remote access.
Suitable for **paper trading and development only** — not for live funded accounts.

## Prerequisites
- Laptop always plugged in, on stable WiFi
- Disable sleep/hibernation
- ngrok account (free tier works for monitoring, paid for 24/7)

## 1. Prevent Sleep
```powershell
# Disable sleep on AC power
powercfg /change standby-timeout-ac 0
powercfg /change hibernate-timeout-ac 0
powercfg /change monitor-timeout-ac 30  # screen off after 30 min
```

## 2. Install ngrok
```powershell
# Download from https://ngrok.com/download
# Or via winget:
winget install ngrok.ngrok

# Authenticate
ngrok config add-authtoken <YOUR_TOKEN>
```

## 3. Expose the Daemon
```powershell
# Start the daemon (when built)
.\target\release\dococ.exe daemon --port 8080

# In another terminal, expose via ngrok
ngrok http 8080
```

## 4. Keep ngrok Running
For free tier: sessions expire after 2 hours. For paper trading, restart manually.
For paid tier ($8/mo): sessions don't expire.

## 5. Auto-Start on Boot
Create a Windows Task Scheduler entry:
```powershell
# Run as Administrator
$action = New-ScheduledTaskAction -Execute "powershell.exe" -Argument "-File C:\dococ\start_daemon.ps1"
$trigger = New-ScheduledTaskTrigger -AtStartup
$settings = New-ScheduledTaskSettingsSet -AllowStartIfOnBatteries -DontStopIfGoingOnBatteries -StartWhenAvailable
Register-ScheduledTask -TaskName "DocOcDaemon" -Action $action -Trigger $trigger -Settings $settings -RunLevel Highest
```

## 6. start_daemon.ps1
```powershell
$env:DOCOC_LAYA_MODEL = "C:\dococ\models\laya\multilingual"
Set-Location C:\dococ
.\target\release\dococ.exe daemon --port 8080
```

## Limitations
- Laptop must stay on and awake
- Windows updates may restart the machine
- ngrok free tier: 2-hour sessions, 1 tunnel
- No UPS — power outage = daemon down
- Thermal throttling under sustained load
- Not suitable for live trading with funded accounts

## For Production
Move to a real VPS (Hetzner €3.79/mo, Oracle Always Free, or similar) before going live.
The Rust binary is portable — just copy the binary + model to the VPS.