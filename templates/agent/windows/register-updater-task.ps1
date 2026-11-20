# Register daily Zyvor GuestAgent signed update check (Windows Task Scheduler).
# Run as Administrator after installing zyvor-guest-agent.exe.
$ErrorActionPreference = "Stop"

$agentPath = "$env:ProgramFiles\Zyvor\VM Tools\zyvor-guest-agent.exe"
if (-not (Test-Path $agentPath)) {
    Write-Error "Agent not found at $agentPath"
}

$taskName = "ZyvorGuestAgentUpdater"
$action = New-ScheduledTaskAction -Execute $agentPath -Argument "--scheduled-update"
$trigger = New-ScheduledTaskTrigger -Daily -At 3:15AM -RandomDelay (New-TimeSpan -Hours 1)
$principal = New-ScheduledTaskPrincipal -UserId "SYSTEM" -RunLevel Highest
$settings = New-ScheduledTaskSettingsSet -AllowStartIfOnBatteries -DontStopIfGoingOnBatteries

Unregister-ScheduledTask -TaskName $taskName -Confirm:$false -ErrorAction SilentlyContinue
Register-ScheduledTask -TaskName $taskName -Action $action -Trigger $trigger -Principal $principal -Settings $settings -Description "Zyvor GuestAgent signed self-update" | Out-Null
Write-Host "Registered scheduled task $taskName (daily --scheduled-update)"
