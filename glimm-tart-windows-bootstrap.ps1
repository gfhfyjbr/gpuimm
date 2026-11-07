$ErrorActionPreference = 'Stop'
$root = 'Z:\workspace'
Start-Transcript -Path "$root\glimm-windows-bootstrap.log" -Force
try {
    & pnputil.exe /add-driver "$root\glimm-windows-viosock\viosock.inf" /install
    if ($LASTEXITCODE -notin 0, 3010) { throw "viosock installation failed: $LASTEXITCODE" }
    $directory = 'C:\Program Files\Tart'
    New-Item -ItemType Directory -Path $directory -Force | Out-Null
    Copy-Item "$root\glimm-tart-guest-agent.exe" "$directory\tart-guest-agent.exe" -Force
    $action = New-ScheduledTaskAction -Execute "$env:SystemRoot\System32\conhost.exe" -Argument '--headless "C:\Program Files\Tart\tart-guest-agent.exe" --run-agent'
    $trigger = New-ScheduledTaskTrigger -AtLogOn -User $env:USERNAME
    $settings = New-ScheduledTaskSettingsSet -ExecutionTimeLimit ([TimeSpan]::Zero) -RestartCount 999 -RestartInterval (New-TimeSpan -Minutes 1)
    $principal = New-ScheduledTaskPrincipal -UserId $env:USERNAME -LogonType Interactive -RunLevel Highest
    Register-ScheduledTask -TaskName 'Tart Guest Agent' -Action $action -Trigger $trigger -Settings $settings -Principal $principal -Force | Out-Null
    Start-ScheduledTask -TaskName 'Tart Guest Agent'
    Get-Service VirtioSocketWSP
    Get-CimInstance Win32_OperatingSystem | Select-Object Caption, Version, OSArchitecture
} finally {
    Stop-Transcript
}
