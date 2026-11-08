# Wspolne funkcje dla skryptow spike'u (h). Dolaczane przez: . "$PSScriptRoot\lib-common.ps1"
# Nie uruchamiaj tego pliku bezposrednio.

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

function Get-Median {
    param([double[]]$Values)
    $s = @($Values | Sort-Object)
    $n = $s.Count
    if ($n -eq 0) { return 0 }
    if ($n % 2 -eq 1) { return $s[[int][math]::Floor($n / 2)] }
    return ($s[$n / 2 - 1] + $s[$n / 2]) / 2
}

function Get-WavDurationSec {
    # Dlugosc pliku WAV (PCM) w sekundach z naglowka RIFF: pola byteRate i rozmiar chunku "data".
    param([string]$Path)
    $fs = [System.IO.File]::OpenRead($Path)
    try {
        $br = New-Object System.IO.BinaryReader($fs)
        $riff = [System.Text.Encoding]::ASCII.GetString($br.ReadBytes(4))
        if ($riff -ne 'RIFF') { throw "To nie jest plik WAV (RIFF): $Path" }
        [void]$br.ReadUInt32()   # rozmiar pliku
        [void]$br.ReadBytes(4)   # "WAVE"
        $byteRate = 0
        while ($fs.Position -lt $fs.Length - 8) {
            $id = [System.Text.Encoding]::ASCII.GetString($br.ReadBytes(4))
            $size = $br.ReadUInt32()
            if ($id -eq 'fmt ') {
                [void]$br.ReadUInt16()          # format
                [void]$br.ReadUInt16()          # kanaly
                [void]$br.ReadUInt32()          # sampleRate
                $byteRate = $br.ReadUInt32()
                $fs.Seek($size - 12, [System.IO.SeekOrigin]::Current) | Out-Null
            }
            elseif ($id -eq 'data') {
                if ($byteRate -eq 0) { throw "Brak chunku fmt przed data: $Path" }
                return [math]::Round($size / $byteRate, 2)
            }
            else {
                $fs.Seek($size + ($size % 2), [System.IO.SeekOrigin]::Current) | Out-Null
            }
        }
        throw "Nie znaleziono chunku data: $Path"
    }
    finally { $fs.Dispose() }
}

function Get-GpuVendor {
    # 'nvidia' gdy jest nvidia-smi, inaczej 'other' (AMD/Intel -> liczniki wydajnosci).
    if (Get-Command nvidia-smi -ErrorAction SilentlyContinue) { return 'nvidia' }
    return 'other'
}

function Get-VramUsedMB {
    # Chwilowe zuzycie dedykowanej pamieci GPU [MB] (suma po adapterach).
    param([string]$Vendor)
    if ($Vendor -eq 'nvidia') {
        $v = (& nvidia-smi --query-gpu=memory.used --format=csv,noheader,nounits | Select-Object -First 1).Trim()
        return [double]$v
    }
    $c = Get-Counter '\GPU Adapter Memory(*)\Dedicated Usage' -ErrorAction SilentlyContinue
    if ($null -eq $c) { return -1 }
    $sum = ($c.CounterSamples | Measure-Object -Property CookedValue -Sum).Sum
    return [math]::Round($sum / 1MB, 0)
}

function Start-VramSampler {
    # Uruchamia zadanie w tle probkujace VRAM co 500 ms; maksimum zapisuje do pliku.
    param([string]$Vendor, [string]$StateFile)
    Set-Content -Path $StateFile -Value '0'
    $stopFile = "$StateFile.stop"
    if (Test-Path $stopFile) { Remove-Item $stopFile }
    $job = Start-Job -ArgumentList $Vendor, $StateFile, $stopFile -ScriptBlock {
        param($Vendor, $StateFile, $StopFile)
        $max = 0.0
        while (-not (Test-Path $StopFile)) {
            try {
                if ($Vendor -eq 'nvidia') {
                    $v = [double](& nvidia-smi --query-gpu=memory.used --format=csv,noheader,nounits | Select-Object -First 1).Trim()
                }
                else {
                    $c = Get-Counter '\GPU Adapter Memory(*)\Dedicated Usage' -ErrorAction SilentlyContinue
                    $v = if ($null -eq $c) { -1 } else { [math]::Round((($c.CounterSamples | Measure-Object -Property CookedValue -Sum).Sum) / 1MB, 0) }
                }
                if ($v -gt $max) { $max = $v; Set-Content -Path $StateFile -Value ([string]$max) }
            }
            catch { }
            Start-Sleep -Milliseconds 500
        }
    }
    return [pscustomobject]@{ Job = $job; StateFile = $StateFile; StopFile = $stopFile }
}

function Stop-VramSampler {
    # Zatrzymuje probkowanie i zwraca szczyt VRAM [MB].
    param($Sampler)
    Set-Content -Path $Sampler.StopFile -Value '1'
    Wait-Job -Job $Sampler.Job -Timeout 10 | Out-Null
    Remove-Job -Job $Sampler.Job -Force -ErrorAction SilentlyContinue
    $v = (Get-Content -Path $Sampler.StateFile -Raw).Trim()
    Remove-Item $Sampler.StateFile, $Sampler.StopFile -ErrorAction SilentlyContinue
    return [double]$v
}

function Get-EnvHeaderLines {
    # Linie naglowka z opisem srodowiska (do pliku wynikow).
    $os = Get-CimInstance Win32_OperatingSystem
    $cpu = (Get-CimInstance Win32_Processor | Select-Object -First 1)
    $gpus = (Get-CimInstance Win32_VideoController | ForEach-Object { "$($_.Name) (sterownik $($_.DriverVersion))" }) -join '; '
    $ramGB = [math]::Round($os.TotalVisibleMemorySize / 1MB, 1)
    $power = 'n/d'
    $bat = Get-CimInstance Win32_Battery -ErrorAction SilentlyContinue
    if ($bat) { $power = if ($bat.BatteryStatus -eq 2) { 'zasilacz' } else { 'bateria' } }
    @(
        "| CPU / RAM | $($cpu.Name) ($($cpu.NumberOfCores)c/$($cpu.NumberOfLogicalProcessors)t) / $ramGB GB |",
        "| GPU / sterownik | $gpus |",
        "| Windows | $($os.Caption) $($os.Version) |",
        "| Zasilanie | $power |",
        "| Procesory logiczne dostepne dla tego procesu | $([Environment]::ProcessorCount) (affinity: 0x$(([int64][System.Diagnostics.Process]::GetCurrentProcess().ProcessorAffinity).ToString('X'))) |"
    )
}

function Add-ResultRow {
    # Dopisuje wiersz do sekcji tabeli w pliku wynikow; tworzy plik z szablonu, gdy nie istnieje.
    param([string]$MdPath, [string]$Section, [string]$Row, [string]$TemplatePath)
    if (-not (Test-Path $MdPath)) {
        $tpl = Get-Content -Path $TemplatePath -Raw
        $tpl = $tpl -replace '<maszyna> — <RRRR-MM-DD>', ((Split-Path $MdPath -Leaf) -replace '\.md$', '')
        $envLines = (Get-EnvHeaderLines) -join "`n"
        $tpl = $tpl -replace '\| CPU / RAM \| \|', $envLines
        Set-Content -Path $MdPath -Value $tpl -Encoding UTF8
    }
    $lines = [System.Collections.Generic.List[string]](Get-Content -Path $MdPath)
    $secIdx = -1
    for ($i = 0; $i -lt $lines.Count; $i++) { if ($lines[$i].Trim() -eq $Section) { $secIdx = $i; break } }
    if ($secIdx -lt 0) { throw "Nie znaleziono sekcji '$Section' w $MdPath" }
    # Znajdz koniec tabeli w tej sekcji (pierwsza pusta linia po naglowku tabeli).
    $insertAt = -1
    $inTable = $false
    for ($i = $secIdx + 1; $i -lt $lines.Count; $i++) {
        if ($lines[$i].StartsWith('|')) { $inTable = $true; continue }
        if ($inTable) { $insertAt = $i; break }
    }
    if ($insertAt -lt 0) { $insertAt = $lines.Count }
    # Usun pusty wiersz-wzorzec "| | | ... |" tuz przed miejscem wstawienia.
    if ($insertAt -gt 0 -and ($lines[$insertAt - 1] -match '^\|(\s*\|)+\s*$')) { $lines.RemoveAt($insertAt - 1); $insertAt-- }
    $lines.Insert($insertAt, $Row)
    Set-Content -Path $MdPath -Value ($lines -join "`n") -Encoding UTF8
}
