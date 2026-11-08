<#
.SYNOPSIS
  Spike (h): llama-bench (pp512 / tg128) dla modeli 4B i 8B Q4_K_M na Vulkan / CUDA / CPU, ze szczytem VRAM.
.DESCRIPTION
  Uruchamiaj z katalogu glownego repo w PowerShell 7, np.:
    pwsh -NoProfile -File .\evals\spikes\h-sprzet\bench-llama.ps1 -Machine desktop -Backend vulkan -LlamaDir "$HOME\alfa-spikes\bin\llama-vulkan" -Models "$HOME\alfa-spikes\models\a.gguf","$HOME\alfa-spikes\models\b.gguf"
#>
[CmdletBinding()]
param(
    # Nazwa maszyny (desktop, laptop, desktop-emu, laptop-emu)
    [string]$Machine = 'desktop',
    # Backend paczki: vulkan, cuda, albo cpu (-ngl 0 na dowolnej paczce)
    [ValidateSet('vulkan', 'cuda', 'cpu')]
    [string]$Backend = 'vulkan',
    # Katalog z llama-bench.exe
    [string]$LlamaDir = "$HOME\alfa-spikes\bin\llama-vulkan",
    # Lista plikow GGUF (domyslnie wszystkie *.gguf w katalogu modeli)
    [string[]]$Models = @(),
    # Liczba warstw na GPU (99 = wszystkie); dla cpu wymuszane 0
    [int]$NGpuLayers = 99,
    # Watki CPU (6 = baseline)
    [int]$Threads = 6,
    # Powtorzenia w llama-bench
    [int]$Repetitions = 5,
    # Katalog wynikow
    [string]$OutDir = 'evals\spikes\h-sprzet\results'
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
. "$PSScriptRoot\lib-common.ps1"

$benchExe = Join-Path $LlamaDir 'llama-bench.exe'
if (-not (Test-Path $benchExe)) { throw "Brak $benchExe - rozpakuj wydanie llama.cpp (README, sekcja E)." }
if ($Models.Count -eq 0) {
    $Models = @(Get-ChildItem -Path "$HOME\alfa-spikes\models" -Filter *.gguf -ErrorAction SilentlyContinue | ForEach-Object { $_.FullName })
    if ($Models.Count -eq 0) { throw "Podaj -Models albo wgraj pliki .gguf do $HOME\alfa-spikes\models (README, sekcja F)." }
}
$ngl = if ($Backend -eq 'cpu') { 0 } else { $NGpuLayers }
$vendor = Get-GpuVendor
$date = Get-Date -Format 'yyyy-MM-dd'
New-Item -ItemType Directory -Force -Path $OutDir | Out-Null
$mdPath = Join-Path $OutDir "$Machine-$date.md"
$logPath = Join-Path $OutDir "$Machine-$date-llama-$Backend.log"
"=== bench-llama $Machine $Backend $(Get-Date -Format s) ===" | Add-Content $logPath

$factor = 1.0
if ($Machine -like 'desktop*') { $factor = if ($Backend -eq 'cpu') { 1.25 } else { 2.2 } }
$emu = if ($Machine -like '*-emu') { 'tak' } else { 'nie' }

foreach ($m in $Models) {
    if (-not (Test-Path $m)) { Write-Warning "Pomijam, brak pliku: $m"; continue }
    $name = Split-Path $m -Leaf
    $sizeGB = [math]::Round((Get-Item $m).Length / 1GB, 2)
    Write-Host "llama-bench: $name ($sizeGB GB), -ngl $ngl, -t $Threads ..."
    $sampler = Start-VramSampler -Vendor $vendor -StateFile (Join-Path $OutDir "vram-llama-$PID.tmp")
    $out = ''
    $code = 0
    try {
        $out = & $benchExe -m $m -p 512 -n 128 -ngl $ngl -t $Threads -r $Repetitions -o md 2>&1 | Out-String
        $code = $LASTEXITCODE
    }
    finally {
        $peak = Stop-VramSampler -Sampler $sampler
    }
    "--- $name exit=$code ---" | Add-Content $logPath
    $out | Add-Content $logPath

    # Wyciagnij tok/s z tabeli Markdown llama-bench: kolumny "test" i "t/s" (np. "pp512 | 1234.56 +- 1.2").
    $pp = 'n/d'; $tg = 'n/d'
    foreach ($line in ($out -split "`n")) {
        if ($line -match '\|\s*pp512\s*\|\s*([\d\.]+)') { $pp = [math]::Round([double]$Matches[1], 1) }
        if ($line -match '\|\s*tg128\s*\|\s*([\d\.]+)') { $tg = [math]::Round([double]$Matches[1], 1) }
    }
    $note = if ($code -ne 0) { "BLAD exit=$code (patrz log; OOM?)" } else { '' }
    if ($out -match 'out of memory|OOM|DeviceLost') { $note += ' pamiec GPU: OOM/DeviceLost w logu' }
    $tgCorr = if ($tg -is [double]) { "$([math]::Round([double]$tg / $factor, 1)) (/x$factor)" } else { 'n/d' }
    Write-Host "  pp512: $pp tok/s, tg128: $tg tok/s, szczyt VRAM: $peak MB $note"
    $row = "| $Machine | $emu | $Backend | $name | $sizeGB | $ngl | $Threads | $pp | $tg | $tgCorr | $peak | $note |"
    Add-ResultRow -MdPath $mdPath -Section '## llama.cpp (`llama-bench -p 512 -n 128`)' -Row $row -TemplatePath (Join-Path $PSScriptRoot 'results\TEMPLATE.md')
}

Write-Host ''
Write-Host "Dopisano wiersze do: $mdPath (log: $logPath)"
Write-Host 'Uwaga: dla LLM korekta GPU x2,2 obniza tok/s (dzielenie), bo baseline ma wolniejsza pamiec GPU.'
