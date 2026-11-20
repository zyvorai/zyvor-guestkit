# Zeus VM Tools — Windows guest agent installer
# Run as Administrator inside the guest VM.
param(
    [string]$AgentUrl = $env:ZYVOR_AGENT_URL,
    [string]$ConfigUrl = $env:ZYVOR_AGENT_CONFIG_URL,
    [string]$PolicyUrl = $env:ZYVOR_AGENT_POLICY_URL,
    [string]$InstallDir = "$env:ProgramFiles\Zyvor\VM Tools",
    [string]$ConfigDir = "$env:ProgramData\zyvor"
)

$ErrorActionPreference = "Stop"

if (-not $AgentUrl) {
    Write-Error "Set -AgentUrl or ZYVOR_AGENT_URL to the guestkitd.exe download URL."
}

New-Item -ItemType Directory -Force -Path $InstallDir | Out-Null
New-Item -ItemType Directory -Force -Path $ConfigDir | Out-Null
$agentPath = Join-Path $InstallDir "guestkitd.exe"

Write-Host "Downloading Zeus VM Tools agent from $AgentUrl"
Invoke-WebRequest -Uri $AgentUrl -OutFile $agentPath -UseBasicParsing

$configPath = Join-Path $ConfigDir "guest-agent.toml"
if ($ConfigUrl) {
    Write-Host "Downloading guest-agent.toml from $ConfigUrl"
    Invoke-WebRequest -Uri $ConfigUrl -OutFile $configPath -UseBasicParsing
} elseif (-not (Test-Path $configPath)) {
    @"
zeus_url = ""
interval_secs = 60
"@ | Set-Content -Path $configPath -Encoding UTF8
}

$policyPath = Join-Path $ConfigDir "agent-policy.yaml"
if ($PolicyUrl) {
    Write-Host "Downloading agent-policy.yaml from $PolicyUrl"
    Invoke-WebRequest -Uri $PolicyUrl -OutFile $policyPath -UseBasicParsing
}

$svcName = "GuestKitAgent"
$legacySvcName = "ZyvorGuestAgent"
# Remove the pre-rebrand service if present
if (Get-Service $legacySvcName -ErrorAction SilentlyContinue) {
    Stop-Service $legacySvcName -Force -ErrorAction SilentlyContinue
    sc.exe delete $legacySvcName | Out-Null
}
$svcDisplay = "Zeus VM Tools Guest Agent"
$binPath = "`"$agentPath`" --service"

if (Get-Service -Name $svcName -ErrorAction SilentlyContinue) {
    Stop-Service -Name $svcName -Force -ErrorAction SilentlyContinue
    sc.exe delete $svcName | Out-Null
    Start-Sleep -Seconds 2
}

New-Service -Name $svcName -BinaryPathName $binPath -DisplayName $svcDisplay -StartupType Automatic | Out-Null
Start-Service -Name $svcName

Write-Host "Zeus VM Tools installed. Service $svcName is running."
Write-Host "Config: $configPath"

$updaterScript = Join-Path $InstallDir "register-updater-task.ps1"
if (Test-Path $updaterScript) {
    Write-Host "Registering daily signed update task…"
    & $updaterScript
}
