<#
.SYNOPSIS
  Spike (f): pomiar Private Working Set drzewa procesow Tauri/WebView2 (1 i 3 okna) oraz czasu startu.
.DESCRIPTION
  Uruchamiaj z katalogu glownego repo w PowerShell 7:
    pwsh -NoProfile -File .\evals\spikes\f-ram-tauri\measure-ram.ps1 -Machine desktop
  Wyniki: results\<Machine>-<data>.md i .csv. Szczegoly w README.md obok.
#>
[CmdletBinding()]
param(
    # Nazwa maszyny do nazwy pliku wynikow: desktop, laptop, desktop-emu, laptop-emu
    [string]$Machine = 'desktop',
    # Sciezka do binarium powloki (release). Wzgledna = wzgledem katalogu glownego repo.
    [string]$ExePath = 'apps\desktop\src-tauri\target\release\alfa-desktop.exe',
    # Liczba pomiarow startu (pierwszy = zimny, kolejne = cieple)
    [int]$StartRuns = 5,
    # Liczba probek RAM w kazdej fazie
    [int]$Samples = 10,
    # Odstep miedzy probkami [s]
    [int]$IntervalSec = 2,
    # Czas "uspokojenia" po starcie, zanim zacznie sie probkowanie [s]
    [int]$SettleSec = 20,
    # Maksymalny czas oczekiwania na okno glowne [s]
    [int]$WindowTimeoutSec = 30,
    # Pomin faze 3 okien
    [switch]$NoInteractive,
    # Zamiast prosic o otwarcie okien, uruchom 3 osobne instancje exe (gorna granica)
    [switch]$ThreeInstances,
    # Katalog wynikow
    [string]$OutDir = 'evals\spikes\f-ram-tauri\results'
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

# --- Pomocnicze ---------------------------------------------------------------

function Get-ProcessTreePids {
    # Zwraca PID-y procesu i wszystkich jego potomkow (BFS po ParentProcessId).
    param([int[]]$RootPids)
    $all = Get-CimInstance -ClassName Win32_Process | Select-Object ProcessId, ParentProcessId, Name
    $byParent = @{}
    foreach ($p in $all) {
        if (-not $byParent.ContainsKey([int]$p.ParentProcessId)) { $byParent[[int]$p.ParentProcessId] = @() }
        $byParent[[int]$p.ParentProcessId] += [int]$p.ProcessId
    }
    $seen = [System.Collections.Generic.HashSet[int]]::new()
    $queue = [System.Collections.Generic.Queue[int]]::new()
    foreach ($r in $RootPids) { [void]$queue.Enqueue($r) }
    while ($queue.Count -gt 0) {
        $pid_ = $queue.Dequeue()
        if (-not $seen.Add($pid_)) { continue }
        if ($byParent.ContainsKey($pid_)) {
            foreach ($c in $byParent[$pid_]) { [void]$queue.Enqueue($c) }
        }
    }
    return @($seen)
}

function Get-TreeSample {
    # Jedna probka: suma Private Working Set [MB] i CPU [%] dla podanych PID-ow.
    param([int[]]$Pids)
    $perf = Get-CimInstance -ClassName Win32_PerfFormattedData_PerfProc_Process |
        Where-Object { $Pids -contains [int]$_.IDProcess }
    $privateBytes = ($perf | Measure-Object -Property WorkingSetPrivate -Sum).Sum
    $cpu = ($perf | Measure-Object -Property PercentProcessorTime -Sum).Sum
    $logical = [Environment]::ProcessorCount
    [pscustomobject]@{
        PrivateWsMB = [math]::Round(($privateBytes / 1MB), 1)
        CpuPercent  = [math]::Round(($cpu / $logical), 2)   # PercentProcessorTime sumuje sie po rdzeniach
        Processes   = @($perf).Count
    }
}

function Get-Median {
    param([double[]]$Values)
    $s = $Values | Sort-Object
    $n = $s.Count
    if ($n -eq 0) { return 0 }
    if ($n % 2 -eq 1) { return $s[[int][math]::Floor($n / 2)] }
    return ($s[$n / 2 - 1] + $s[$n / 2]) / 2
}

function Start-AppAndWaitForWindow {
    # Uruchamia exe i czeka na okno glowne. Zwraca proces i czas do okna [ms].
    param([string]$Path, [int]$TimeoutSec)
    $sw = [System.Diagnostics.Stopwatch]::StartNew()
    $proc = Start-Process -FilePath $Path -PassThru
    $deadline = (Get-Date).AddSeconds($TimeoutSec)
    while ((Get-Date) -lt $deadline) {
        $proc.Refresh()
        if ($proc.HasExited) { throw "Proces zakonczyl sie przed pokazaniem okna (kod $($proc.ExitCode))." }
        if ($proc.MainWindowHandle -ne 0) { break }
        Start-Sleep -Milliseconds 20
    }
    $sw.Stop()
    if ($proc.MainWindowHandle -eq 0) { throw "Okno glowne nie pojawilo sie w ciagu $TimeoutSec s." }
    return [pscustomobject]@{ Process = $proc; Ms = [int]$sw.ElapsedMilliseconds }
}

function Stop-Tree {
    param([int]$RootPid)
    $pids = Get-ProcessTreePids -RootPids @($RootPid)
    foreach ($p in $pids) { Stop-Process -Id $p -Force -ErrorAction SilentlyContinue }
    Start-Sleep -Seconds 2
}

function Measure-Phase {
    # Probkuje drzewo procesow i zwraca statystyki fazy.
    param([string]$Name, [int[]]$RootPids)
    Write-Host "  [$Name] czekam $SettleSec s na uspokojenie..."
    Start-Sleep -Seconds $SettleSec
    $rows = @()
    for ($i = 1; $i -le $Samples; $i++) {
        $pids = Get-ProcessTreePids -RootPids $RootPids
        $s = Get-TreeSample -Pids $pids
        $rows += $s
        Write-Host ("  [{0}] probka {1}/{2}: {3} MB, CPU {4} %, procesow {5}" -f $Name, $i, $Samples, $s.PrivateWsMB, $s.CpuPercent, $s.Processes)
        Start-Sleep -Seconds $IntervalSec
    }
    [pscustomobject]@{
        Name       = $Name
        MedianMB   = [math]::Round((Get-Median -Values ($rows | ForEach-Object { [double]$_.PrivateWsMB })), 1)
        MaxMB      = ($rows | Measure-Object -Property PrivateWsMB -Maximum).Maximum
        CpuMedian  = [math]::Round((Get-Median -Values ($rows | ForEach-Object { [double]$_.CpuPercent })), 2)
        Processes  = ($rows | Measure-Object -Property Processes -Maximum).Maximum
        Samples    = $rows
    }
}

# --- Informacje o srodowisku --------------------------------------------------

$exe = (Resolve-Path -Path $ExePath).Path
$mode = if ($exe -match '\\release\\') { 'release' } else { 'dev/inne' }
if ($ThreeInstances) { $mode += ' + 3 instancje' }
$osInfo = Get-CimInstance Win32_OperatingSystem
$osText = "$($osInfo.Caption) $($osInfo.Version)"
$gpu = (Get-CimInstance Win32_VideoController | Select-Object -First 1)
$gpuText = "$($gpu.Name) / $($gpu.DriverVersion)"
$wv2 = 'nieznana'
foreach ($k in @('HKLM:\SOFTWARE\WOW6432Node\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}',
                 'HKCU:\SOFTWARE\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}')) {
    if (Test-Path $k) { $wv2 = (Get-ItemProperty $k).pv; break }
}

$date = Get-Date -Format 'yyyy-MM-dd'
New-Item -ItemType Directory -Force -Path $OutDir | Out-Null
$mdPath = Join-Path $OutDir "$Machine-$date.md"
$csvPath = Join-Path $OutDir "$Machine-$date.csv"

Write-Host "Spike (f) - maszyna: $Machine, exe: $exe, tryb: $mode"
Write-Host "Windows: $osText | WebView2: $wv2 | GPU: $gpuText"

# --- Faza 0: czasy startu -----------------------------------------------------

$startTimes = @()
for ($r = 1; $r -le $StartRuns; $r++) {
    $res = Start-AppAndWaitForWindow -Path $exe -TimeoutSec $WindowTimeoutSec
    $startTimes += $res.Ms
    Write-Host ("  start {0}/{1}: {2} ms" -f $r, $StartRuns, $res.Ms)
    Start-Sleep -Seconds 3
    Stop-Tree -RootPid $res.Process.Id
}
$coldMs = $startTimes[0]
$warmMs = if ($startTimes.Count -gt 1) { [int](Get-Median -Values ($startTimes[1..($startTimes.Count - 1)] | ForEach-Object { [double]$_ })) } else { 0 }

# --- Faza 1: jedno okno -------------------------------------------------------

$one = Start-AppAndWaitForWindow -Path $exe -TimeoutSec $WindowTimeoutSec
$rootPids = @($one.Process.Id)
$phase1 = Measure-Phase -Name '1 okno' -RootPids $rootPids

# --- Faza 2: trzy okna --------------------------------------------------------

$phase3 = $null
if (-not $NoInteractive) {
    if ($ThreeInstances) {
        $extra = @()
        for ($i = 0; $i -lt 2; $i++) {
            $e = Start-AppAndWaitForWindow -Path $exe -TimeoutSec $WindowTimeoutSec
            $extra += $e.Process.Id
        }
        $rootPids = $rootPids + $extra
    }
    else {
        Write-Host ''
        Write-Host 'Otworz teraz DWA dodatkowe okna w aplikacji (np. Ctrl+Shift+N dwa razy).'
        Read-Host 'Gdy 3 okna sa widoczne, nacisnij Enter'
    }
    $phase3 = Measure-Phase -Name '3 okna' -RootPids $rootPids
}

foreach ($p in $rootPids) { Stop-Tree -RootPid $p }

# --- Zapis --------------------------------------------------------------------

$p3med = if ($phase3) { $phase3.MedianMB } else { 'n/d' }
$p3max = if ($phase3) { $phase3.MaxMB } else { 'n/d' }
$p3cpu = if ($phase3) { $phase3.CpuMedian } else { 'n/d' }
$p3proc = if ($phase3) { $phase3.Processes } else { 'n/d' }

$md = @(
    "# Spike (f) - $Machine - $date",
    '',
    "Exe: ``$exe``  ",
    "Tryb: $mode  ",
    "Windows: $osText | WebView2: $wv2 | GPU/sterownik: $gpuText  ",
    "Parametry: StartRuns=$StartRuns, Samples=$Samples, IntervalSec=$IntervalSec, SettleSec=$SettleSec",
    '',
    '| Maszyna | Tryb | Zimny start [ms] | Ciepły start p50 [ms] | 1 okno: Private WS mediana [MB] | 1 okno: max [MB] | 3 okna: mediana [MB] | 3 okna: max [MB] | Idle CPU 1 okno [%] | Idle CPU 3 okna [%] | Procesy w drzewie (1/3) | Windows | WebView2 | Sterownik GPU |',
    '|---|---|---|---|---|---|---|---|---|---|---|---|---|---|',
    "| $Machine | $mode | $coldMs | $warmMs | $($phase1.MedianMB) | $($phase1.MaxMB) | $p3med | $p3max | $($phase1.CpuMedian) | $p3cpu | $($phase1.Processes)/$p3proc | $osText | $wv2 | $gpuText |",
    '',
    "Czasy startu [ms]: $($startTimes -join ', ')",
    '',
    'Progi wstepne (PLAN 3.4): zimny start do okna <= 1500 ms; idle CPU ~0 %. Budzet RAM ustala ADR na podstawie tego pomiaru.',
    '',
    'Uwagi (wypelnij recznie): '
)
$md -join "`n" | Set-Content -Path $mdPath -Encoding UTF8

$csvRows = @()
$i = 0
foreach ($t in $startTimes) { $i++; $csvRows += [pscustomobject]@{ machine = $Machine; phase = 'start'; sample = $i; value_ms = $t; private_ws_mb = ''; cpu_percent = ''; processes = '' } }
$i = 0
foreach ($s in $phase1.Samples) { $i++; $csvRows += [pscustomobject]@{ machine = $Machine; phase = '1-okno'; sample = $i; value_ms = ''; private_ws_mb = $s.PrivateWsMB; cpu_percent = $s.CpuPercent; processes = $s.Processes } }
if ($phase3) {
    $i = 0
    foreach ($s in $phase3.Samples) { $i++; $csvRows += [pscustomobject]@{ machine = $Machine; phase = '3-okna'; sample = $i; value_ms = ''; private_ws_mb = $s.PrivateWsMB; cpu_percent = $s.CpuPercent; processes = $s.Processes } }
}
$csvRows | Export-Csv -Path $csvPath -NoTypeInformation -Encoding UTF8

Write-Host ''
Write-Host "Zapisano: $mdPath"
Write-Host "Zapisano: $csvPath"
Write-Host 'Skopiuj oba pliki z powrotem (albo commit na galezi spikes/f0-wyniki).'
