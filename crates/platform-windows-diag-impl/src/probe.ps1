# Sonda parametrów komputera (pc-diag). Tylko odczyt. Każda część osobno: błąd części trafia do
# luki (klucz części + komunikat), reszta zapisu powstaje dalej. Wynik: wiersz JSON na część.
# Bez adresów IP/MAC i bez pełnych numerów seryjnych (tylko ostatnie 4 znaki).
$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'
# Limit jednego zapytania CIM: zawieszona klasa (1.11: bateria przez WMI nie odpowiadała, a sonda
# traciła cały limit 90 s) kończy się luką tylko swojej części, następne części są czytane dalej.
$PSDefaultParameterValues['Get-CimInstance:OperationTimeoutSec'] = 10
[Console]::OutputEncoding = [Text.Encoding]::UTF8
# Każda część trafia na wyjście od razu (`{"k":…,"v":…}`): gdy któraś zawiśnie ponad limit, Alfa
# zatrzymuje sondę i składa zapis z części już odczytanych (`stream.rs`), brakujące — jako luki.
function Emit($key, $value) {
  [Console]::Out.WriteLine((@{ k = $key; v = $value } | ConvertTo-Json -Depth 7 -Compress))
  [Console]::Out.Flush()
}
function Part($key, $name, [scriptblock]$body) {
  $v = try { & $body } catch {
    $msg = $_.Exception.Message
    if ($msg -match '(?i)timed out|timeout|przekroczon') { $msg = 'system nie odpowiedział w 10 s' }
    Emit 'gap' @{ part = $name; error = $msg }; $null
  }
  Emit $key $v
}
function Tail($s) { if ($s) { $t = "$s".Trim(); if ($t.Length -gt 4) { $t.Substring($t.Length - 4) } else { $t } } }

Part 'system' 'system' {
  $o = Get-CimInstance Win32_OperatingSystem
  $cs = Get-CimInstance Win32_ComputerSystem
  $b = Get-CimInstance Win32_BIOS
  @{ caption = $o.Caption; version = $o.Version; build = $o.BuildNumber
     boot = $o.LastBootUpTime.ToUniversalTime().ToString('o')
     total_kb = [int64]$o.TotalVisibleMemorySize; free_kb = [int64]$o.FreePhysicalMemory
     maker = $cs.Manufacturer; model = $cs.Model; bios = $b.SMBIOSBIOSVersion }
}
Part 'cpu' 'cpu' {
  $all = @(Get-CimInstance Win32_Processor)
  $c = $all[0]
  @{ name = "$($c.Name)".Trim()
     cores = ($all | Measure-Object NumberOfCores -Sum).Sum
     threads = ($all | Measure-Object NumberOfLogicalProcessors -Sum).Sum
     max = $c.MaxClockSpeed; cur = $c.CurrentClockSpeed; load = $c.LoadPercentage
     l2 = $c.L2CacheSize; l3 = $c.L3CacheSize; virt = $c.VirtualizationFirmwareEnabled }
}
Part 'modules' 'memory_modules' {
  @(Get-CimInstance Win32_PhysicalMemory | ForEach-Object {
    @{ slot = $_.DeviceLocator; cap = [int64]$_.Capacity; speed = $_.ConfiguredClockSpeed
       maker = $_.Manufacturer; part = "$($_.PartNumber)".Trim() } })
}
Part 'pagefile' 'pagefile' {
  @(Get-CimInstance Win32_PageFileUsage | ForEach-Object { @{ size = $_.AllocatedBaseSize; used = $_.CurrentUsage } })
}
Part 'disks' 'disks' {
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
Part 'volumes' 'volumes' {
  $sys = $env:SystemDrive
  @(Get-CimInstance Win32_LogicalDisk -Filter 'DriveType=2 OR DriveType=3' | ForEach-Object {
    @{ letter = $_.DeviceID; label = $_.VolumeName; fs = $_.FileSystem; size = [int64]$_.Size
       free = [int64]$_.FreeSpace; system = ($_.DeviceID -eq $sys) } })
}
Part 'gpus' 'gpus' {
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
Part 'net' 'network' {
  # Win32_NetworkAdapter zamiast Get-NetAdapter: ten drugi potrafi wisieć ponad limit sondy
  # (31.10 na laptopie właściciela > 80 s). Prędkość tylko połączonej karty (rozłączona podaje
  # wartość maksymalną zamiast zera). Tylko karty z nazwą połączenia; `pnp` (identyfikator
  # urządzenia) odróżnia sprzęt (PCI, USB, SDIO) od kart programowych, które Windows też oznacza
  # jako fizyczne (VirtualBox Host-Only, TAP VPN) — przegląd w świeżym kontekście 1.11.
  @(Get-CimInstance Win32_NetworkAdapter -Filter 'PhysicalAdapter=TRUE AND NetConnectionID IS NOT NULL' | ForEach-Object {
    $up = ($_.NetConnectionStatus -eq 2)
    @{ name = "$($_.NetConnectionID)"; desc = $_.Name; up = $up; pnp = "$($_.PNPDeviceID)"
       bps = $(if ($up -and $_.Speed) { [int64]$_.Speed } else { 0 }); media = "$($_.AdapterType)" } })
}
Part 'battery' 'battery' {
  $b = @(Get-CimInstance Win32_Battery)
  if ($b.Count -gt 0) {
    $wmi = { param($c, $p) try { (Get-CimInstance -Namespace root\wmi $c -OperationTimeoutSec 4 -ErrorAction Stop | Select-Object -First 1).$p } catch { $null } }
    @{ charge = $b[0].EstimatedChargeRemaining; status = $b[0].BatteryStatus
       design = (& $wmi 'BatteryStaticData' 'DesignedCapacity')
       full = (& $wmi 'BatteryFullChargedCapacity' 'FullChargedCapacity')
       cycles = (& $wmi 'BatteryCycleCount' 'CycleCount') }
  }
}
Part 'thermal' 'thermal' {
  @(Get-CimInstance Win32_PerfFormattedData_Counters_ThermalZoneInformation | ForEach-Object {
    $k = if ($_.HighPrecisionTemperature) { $_.HighPrecisionTemperature / 10 } else { $_.Temperature }
    @{ name = $_.Name; c = [math]::Round($k - 273.15, 1) } })
}
Part 'events' 'events' {
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
Emit 'done' $true
