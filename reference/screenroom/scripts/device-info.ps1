# Read-only information to establish the actual Windows performance target.
# Does not collect serial numbers, usernames, IP addresses, or product keys.
$computer = Get-CimInstance Win32_ComputerSystem
$os = Get-CimInstance Win32_OperatingSystem
[ordered]@{
  Windows = $os.Caption
  Version = $os.Version
  Architecture = $os.OSArchitecture
  RAM_GB = [Math]::Round($computer.TotalPhysicalMemory / 1GB, 1)
  CPU = @(Get-CimInstance Win32_Processor | Select-Object -ExpandProperty Name)
  Graphics = @(Get-CimInstance Win32_VideoController | Select-Object Name, DriverVersion, CurrentHorizontalResolution, CurrentVerticalResolution)
  Note = 'Reported GPU resolutions may not identify every attached monitor. Confirm the ultrawide monitor model and selected resolution separately.'
} | ConvertTo-Json -Depth 4
