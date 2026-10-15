# Sonda parametrów komputera (pc-diag). Tylko odczyt. Każda część osobno: błąd części trafia do
# `gaps` (klucz części + komunikat), reszta zapisu powstaje dalej. Wynik: jeden JSON na stdout.
# Bez adresów IP/MAC i bez pełnych numerów seryjnych (tylko ostatnie 4 znaki).
$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'
[Console]::OutputEncoding = [Text.Encoding]::UTF8
$r = [ordered]@{ gaps = @() }
function Part($name, [scriptblock]$body) {
  try { & $body } catch { $script:r.gaps += @{ part = $name; error = $_.Exception.Message }; $null }
}
function Tail($s) { if ($s) { $t = "$s".Trim(); if ($t.Length -gt 4) { $t.Substring($t.Length - 4) } else { $t } } }

$r.system = Part 'system' {
  $o = Get-CimInstance Win32_OperatingSystem
  $cs = Get-CimInstance Win32_ComputerSystem
  $b = Get-CimInstance Win32_BIOS
  @{ caption = $o.Caption; version = $o.Version; build = $o.BuildNumber
     boot = $o.LastBootUpTime.ToUniversalTime().ToString('o')
     total_kb = [int64]$o.TotalVisibleMemorySize; free_kb = [int64]$o.FreePhysicalMemory
     maker = $cs.Manufacturer; model = $cs.Model; bios = $b.SMBIOSBIOSVersion }
}
$r.cpu = Part 'cpu' {
  $all = @(Get-CimInstance Win32_Processor)
  $c = $all[0]
  @{ name = "$($c.Name)".Trim()
     cores = ($all | Measure-Object NumberOfCores -Sum).Sum
     threads = ($all | Measure-Object NumberOfLogicalProcessors -Sum).Sum
     max = $c.MaxClockSpeed; cur = $c.CurrentClockSpeed; load = $c.LoadPercentage
     l2 = $c.L2CacheSize; l3 = $c.L3CacheSize; virt = $c.VirtualizationFirmwareEnabled }
}
$r.modules = Part 'memory_modules' {
  @(Get-CimInstance Win32_PhysicalMemory | ForEach-Object {
    @{ slot = $_.DeviceLocator; cap = [int64]$_.Capacity; speed = $_.ConfiguredClockSpeed
       maker = $_.Manufacturer; part = "$($_.PartNumber)".Trim() } })
}
$r.pagefile = Part 'pagefile' {
  @(Get-CimInstance Win32_PageFileUsage | ForEach-Object { @{ size = $_.AllocatedBaseSize; used = $_.CurrentUsage } })
}
$r.disks = Part 'disks' {
  @(Get-PhysicalDisk | ForEach-Object {
    $d = $_
    $rel = try { $d | Get-StorageReliabilityCounter -ErrorAction Stop } catch { $null }
    $letters = try {
      @(Get-Disk -Number $d.DeviceId -ErrorAction Stop | Get-Partition -ErrorAction Stop |
        Where-Object DriveLetter | ForEach-Object { "$($_.DriveLetter):" })
    } catch { @() }
    @{ number = $d.DeviceId; model = "$($d.FriendlyName)".Trim(); serial = (Tail $d.SerialNumber); media = "$($d.MediaType)"
       bus = "$($d.BusType)"; size = [int64]$d.Size; health = "$($d.HealthStatus)"
       op = "$($d.OperationalStatus)"; wear = $rel.Wear; temp = $rel.Temperature
       hours = $rel.PowerOnHours; rerr = $rel.ReadErrorsTotal; werr = $rel.WriteErrorsTotal
       letters = $letters } })
}
$r.volumes = Part 'volumes' {
  $sys = $env:SystemDrive
  @(Get-CimInstance Win32_LogicalDisk -Filter 'DriveType=2 OR DriveType=3' | ForEach-Object {
    @{ letter = $_.DeviceID; label = $_.VolumeName; fs = $_.FileSystem; size = [int64]$_.Size
       free = [int64]$_.FreeSpace; system = ($_.DeviceID -eq $sys) } })
}
$r.gpus = Part 'gpus' {
  $vram = @{}
  $cls = 'HKLM:\SYSTEM\CurrentControlSet\Control\Class\{4d36e968-e325-11ce-bfc1-08002be10318}'
  Get-ChildItem $cls -ErrorAction SilentlyContinue | ForEach-Object {
    $p = Get-ItemProperty $_.PSPath -ErrorAction SilentlyContinue
    $q = $p.'HardwareInformation.qwMemorySize'
    if ($p.DriverDesc -and $q) { $vram[$p.DriverDesc] = [int64]$q }
  }
  @(Get-CimInstance Win32_VideoController | ForEach-Object {
    $date = if ($_.DriverDate) { $_.DriverDate.ToString('yyyy-MM-dd') } else { $null }
    @{ name = $_.Name; drv = $_.DriverVersion; date = $date; ram = [int64]$_.AdapterRAM
       reg = $vram[$_.Name]; status = $_.Status } })
}
$r.net = Part 'network' {
  @(Get-NetAdapter -Physical | ForEach-Object {
    @{ name = $_.Name; desc = $_.InterfaceDescription; up = ($_.Status -eq 'Up')
       bps = [int64]$_.Speed; media = "$($_.PhysicalMediaType)" } })
}
$r.battery = Part 'battery' {
  $b = @(Get-CimInstance Win32_Battery)
  if ($b.Count -gt 0) {
    $wmi = { param($c, $p) try { (Get-CimInstance -Namespace root\wmi $c -ErrorAction Stop | Select-Object -First 1).$p } catch { $null } }
    @{ charge = $b[0].EstimatedChargeRemaining; status = $b[0].BatteryStatus
       design = (& $wmi 'BatteryStaticData' 'DesignedCapacity')
       full = (& $wmi 'BatteryFullChargedCapacity' 'FullChargedCapacity')
       cycles = (& $wmi 'BatteryCycleCount' 'CycleCount') }
  }
}
$r.thermal = Part 'thermal' {
  @(Get-CimInstance Win32_PerfFormattedData_Counters_ThermalZoneInformation | ForEach-Object {
    $k = if ($_.HighPrecisionTemperature) { $_.HighPrecisionTemperature / 10 } else { $_.Temperature }
    @{ name = $_.Name; c = [math]::Round($k - 273.15, 1) } })
}
$r.events = Part 'events' {
  # Jeden dostawca na zapytanie: brak dostawcy w systemie (np. USBSTOR) pomija tylko jego.
  $since = (Get-Date).AddDays(-7)
  $bad = @(1, 2, 3)
  $filters = @(
    @{ LogName = 'System'; ProviderName = 'Microsoft-Windows-Kernel-Power'; Id = 41 },
    @{ LogName = 'System'; ProviderName = 'EventLog'; Id = 6008 },
    @{ LogName = 'System'; ProviderName = 'Microsoft-Windows-WER-SystemErrorReporting'; Id = 1001 },
    @{ LogName = 'System'; ProviderName = 'Microsoft-Windows-WHEA-Logger'; Level = $bad },
    @{ LogName = 'Application'; ProviderName = 'Application Error'; Id = 1000 }
  )
  foreach ($p in 'disk', 'Ntfs', 'Microsoft-Windows-Ntfs', 'stornvme', 'storahci', 'volmgr', 'UASPStor', 'USBSTOR', 'Microsoft-Windows-StorPort') {
    $filters += @{ LogName = 'System'; ProviderName = $p; Level = $bad }
  }
  $out = @()
  $failed = 0
  foreach ($f in $filters) {
    $f.StartTime = $since
    try {
      $out += @(Get-WinEvent -FilterHashtable $f -MaxEvents 200 -ErrorAction Stop | ForEach-Object {
        $m = "$($_.Message)" -replace '\s+', ' '
        if ($m.Length -gt 200) { $m = $m.Substring(0, 200) }
        @{ at = $_.TimeCreated.ToUniversalTime().ToString('o'); provider = $_.ProviderName; id = $_.Id; msg = $m } })
    } catch { if ($_.FullyQualifiedErrorId -notlike 'NoMatchingEventsFound*') { $failed++ } }
  }
  if ($failed -eq $filters.Count) { throw 'dziennik zdarzeń niedostępny' }
  $out
}
$r | ConvertTo-Json -Depth 6 -Compress
