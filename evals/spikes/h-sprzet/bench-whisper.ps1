<#
.SYNOPSIS
  Spike (h): benchmark whisper.cpp (Vulkan / CUDA / CPU) - whisper-bench, transkrypcja probki, RTF, VRAM, stabilnosc.
.DESCRIPTION
  Uruchamiaj z katalogu glownego repo w PowerShell 7, np.:
    pwsh -NoProfile -File .\evals\spikes\h-sprzet\bench-whisper.ps1 -Machine desktop -Backend vulkan -WhisperDir "$HOME\alfa-spikes\bin\whisper-vulkan"
  Test stabilnosci 1 h: dodaj -StabilityMinutes 60.
#>
[CmdletBinding()]
param(
    # Nazwa maszyny (desktop, laptop, desktop-emu, laptop-emu)
    [string]$Machine = 'desktop',
    # Backend: vulkan, cuda (wg paczki w WhisperDir) albo cpu (ta sama paczka z flaga -ng)
    [ValidateSet('vulkan', 'cuda', 'cpu')]
    [string]$Backend = 'vulkan',
    # Katalog z whisper-cli.exe / whisper-bench.exe
    [string]$WhisperDir = "$HOME\alfa-spikes\bin\whisper-vulkan",
    # Model ggml
    [string]$Model = "$HOME\alfa-spikes\models\ggml-large-v3-turbo-q5_0.bin",
    # Probka WAV 16 kHz mono 16-bit, ok. 60 s
    [string]$Sample = "$HOME\alfa-spikes\samples\pl-60s.wav",
    # Liczba watkow CPU (6 = baseline)
    [int]$Threads = 6,
    # Liczba powtorzen transkrypcji
    [int]$Runs = 5,
    # Jezyk
    [string]$Language = 'pl',
    # Test stabilnosci: minuty transkrypcji w petli (0 = pomin)
    [int]$StabilityMinutes = 0,
    # Katalog wynikow
    [string]$OutDir = 'evals\spikes\h-sprzet\results'
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
. "$PSScriptRoot\lib-common.ps1"

# --- Sprawdzenia wstepne --------------------------------------------------------

function Find-Exe {
    param([string]$Dir, [string[]]$Names)
    foreach ($n in $Names) { $p = Join-Path $Dir $n; if (Test-Path $p) { return $p } }
    throw "Nie znaleziono $($Names -join ' / ') w $Dir. Rozpakuj wydanie whisper.cpp do tego katalogu (README, sekcja B)."
}
$cli = Find-Exe -Dir $WhisperDir -Names @('whisper-cli.exe', 'main.exe')
$bench = Find-Exe -Dir $WhisperDir -Names @('whisper-bench.exe', 'bench.exe')
if (-not (Test-Path $Model)) { throw "Brak modelu: $Model (README, sekcja C)" }
if (-not (Test-Path $Sample)) { throw "Brak probki: $Sample (README, sekcja D)" }

$sampleSec = Get-WavDurationSec -Path $Sample
$modelHash = (Get-FileHash -Path $Model -Algorithm SHA256).Hash.Substring(0, 16)
$vendor = Get-GpuVendor
$date = Get-Date -Format 'yyyy-MM-dd'
New-Item -ItemType Directory -Force -Path $OutDir | Out-Null
$mdPath = Join-Path $OutDir "$Machine-$date.md"
$logPath = Join-Path $OutDir "$Machine-$date-whisper-$Backend.log"
$gpuArgs = @()
if ($Backend -eq 'cpu') { $gpuArgs = @('-ng') }

"=== bench-whisper $Machine $Backend $(Get-Date -Format s) ===" | Add-Content $logPath
"cli=$cli model=$Model sha256[0..16]=$modelHash sample=$Sample ($sampleSec s) threads=$Threads" | Add-Content $logPath
Write-Host "whisper.cpp: $cli | backend: $Backend | model: $(Split-Path $Model -Leaf) | probka: $sampleSec s | GPU: $vendor"

# --- 1. whisper-bench (enkoder) -------------------------------------------------

Write-Host 'whisper-bench (enkoder)...'
$benchOut = & $bench -m $Model -t $Threads @gpuArgs 2>&1 | Out-String
$benchOut | Add-Content $logPath
$encoderMs = 'n/d'
if ($benchOut -match 'encode time\s*=\s*([\d\.]+)\s*ms') { $encoderMs = [math]::Round([double]$Matches[1], 0) }
elseif ($benchOut -match 'total time\s*=\s*([\d\.]+)\s*ms') { $encoderMs = [math]::Round([double]$Matches[1], 0) }
Write-Host "  enkoder: $encoderMs ms"

# --- 2. Transkrypcja N razy z pomiarem VRAM -------------------------------------

$times = @()
$sampler = Start-VramSampler -Vendor $vendor -StateFile (Join-Path $OutDir "vram-$PID.tmp")
try {
    for ($r = 1; $r -le $Runs; $r++) {
        $sw = [System.Diagnostics.Stopwatch]::StartNew()
        $out = & $cli -m $Model -f $Sample -l $Language -t $Threads @gpuArgs 2>&1 | Out-String
        $sw.Stop()
        $code = $LASTEXITCODE
        $sec = [math]::Round($sw.Elapsed.TotalSeconds, 2)
        "--- run $r exit=$code time=$sec s ---" | Add-Content $logPath
        $out | Add-Content $logPath
        if ($code -ne 0) { Write-Warning "Uruchomienie $r zakonczone kodem $code (patrz log)" }
        $times += $sec
        Write-Host ("  transkrypcja {0}/{1}: {2} s (RTF {3})" -f $r, $Runs, $sec, [math]::Round($sec / $sampleSec, 3))
    }
}
finally {
    $peakVram = Stop-VramSampler -Sampler $sampler
}
$medSec = [math]::Round((Get-Median -Values ($times | ForEach-Object { [double]$_ })), 2)
$maxSec = ($times | Measure-Object -Maximum).Maximum
$rtf = [math]::Round($medSec / $sampleSec, 3)
$factor = 1.0
if ($Machine -like 'desktop*') { $factor = if ($Backend -eq 'cpu') { 1.25 } else { 2.2 } }
$rtfCorr = [math]::Round($rtf * $factor, 3)
Write-Host "  mediana: $medSec s, RTF $rtf (po korekcie x$factor : $rtfCorr), szczyt VRAM: $peakVram MB"

# --- 3. Test stabilnosci ----------------------------------------------------------

$stabText = 'pominieto'
if ($StabilityMinutes -gt 0) {
    Write-Host "Test stabilnosci: $StabilityMinutes min w petli (Ctrl+C przerywa)..."
    $deadline = (Get-Date).AddMinutes($StabilityMinutes)
    $iter = 0; $crashes = 0
    $sampler2 = Start-VramSampler -Vendor $vendor -StateFile (Join-Path $OutDir "vram-stab-$PID.tmp")
    try {
        while ((Get-Date) -lt $deadline) {
            $iter++
            $sw = [System.Diagnostics.Stopwatch]::StartNew()
            $out = & $cli -m $Model -f $Sample -l $Language -t $Threads @gpuArgs 2>&1 | Out-String
            $sw.Stop()
            $code = $LASTEXITCODE
            $bad = ($code -ne 0) -or ($out -match 'DeviceLost|ERROR_DEVICE_LOST|out of memory|CUDA error|vk::|failed')
            if ($bad) {
                $crashes++
                "!!! iter $iter exit=$code time=$([math]::Round($sw.Elapsed.TotalSeconds,2)) s" | Add-Content $logPath
                $out | Add-Content $logPath
            }
            elseif ($iter % 10 -eq 0) {
                "iter $iter ok time=$([math]::Round($sw.Elapsed.TotalSeconds,2)) s vram=$(Get-VramUsedMB -Vendor $vendor) MB" | Add-Content $logPath
            }
            Write-Host ("  iter {0}: {1} s, bledow: {2}, pozostalo ~{3} min" -f $iter, [math]::Round($sw.Elapsed.TotalSeconds, 1), $crashes, [math]::Round(($deadline - (Get-Date)).TotalMinutes, 0))
        }
    }
    finally {
        $peakVramStab = Stop-VramSampler -Sampler $sampler2
    }
    $stabText = "$StabilityMinutes min / $iter uruchomien / $crashes bledow (szczyt VRAM $peakVramStab MB)"
    if ($peakVramStab -gt $peakVram) { $peakVram = $peakVramStab }
}

# --- 4. Zapis --------------------------------------------------------------------

$emu = if ($Machine -like '*-emu') { 'tak' } else { 'nie' }
$row = "| $Machine | $emu | $Backend | $(Split-Path $Model -Leaf) ($modelHash) | $Threads | $encoderMs | $medSec | $maxSec | $rtf | $rtfCorr (x$factor) | $peakVram | $stabText | probka $sampleSec s, runs=$Runs |"
Add-ResultRow -MdPath $mdPath -Section '## whisper.cpp' -Row $row -TemplatePath (Join-Path $PSScriptRoot 'results\TEMPLATE.md')
Write-Host ''
Write-Host "Dopisano wiersz do: $mdPath"
Write-Host "Pelny log: $logPath"
