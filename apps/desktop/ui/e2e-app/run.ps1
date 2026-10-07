<#
.SYNOPSIS
    E2E prawdziwej aplikacji (PLAN §4.4): alfa-desktop.exe z cechą `e2e` + Playwright przez CDP do WebView2.
.DESCRIPTION
    Uruchamia zbudowaną powłokę (port CDP 9222 istnieje TYLKO w buildzie z cechą `e2e` — src-tauri/src/cdp.rs),
    czeka na port z limitem, uruchamia `playwright test -c playwright.app.config.ts`, potem zawsze: zrzut pulpitu,
    zakończenie drzewa procesów, kopia logów %LOCALAPPDATA%\Alfa\logs i podsumowanie do $GITHUB_STEP_SUMMARY.
    Artefakty w -OutDir (domyślnie e2e-app\out): screens\*.png, report\index.html, results.json, console.jsonl,
    alfa-desktop.stdout.log / .stderr.log, logs\, cdp-version.json, cdp-targets.json, desktop.png.
    Tylko świeży profil Windows (runner CI): test przechodzi wprowadzenie i zapisuje dane Alfy. Na komputerze
    z istniejącym %LOCALAPPDATA%\Alfa skrypt odmawia startu (chroni dane i klucze właściciela).
    Kod wyjścia: kod Playwrighta; 1 — aplikacja nie wystartowała albo port CDP się nie otworzył.
    Plik w UTF-8 z BOM: bez BOM Windows PowerShell 5.1 psuje polskie znaki w napisach.
.EXAMPLE
    pwsh -NoProfile -File apps\desktop\ui\e2e-app\run.ps1
#>
[CmdletBinding()]
param(
    [string]$Exe,
    [string]$OutDir,
    [int]$Port = 9222,
    [int]$StartTimeoutSec = 180
)

$ErrorActionPreference = 'Stop'
$UiDir = Split-Path -Parent $PSScriptRoot
if (-not $Exe) { $Exe = Join-Path $UiDir '..\src-tauri\target\debug\alfa-desktop.exe' }
if (-not $OutDir) { $OutDir = Join-Path $PSScriptRoot 'out' }
New-Item -ItemType Directory -Force -Path $OutDir | Out-Null
$OutDir = (Resolve-Path $OutDir).Path
$Cdp = "http://127.0.0.1:$Port"
$script:Summary = @('## E2E prawdziwej aplikacji (Playwright przez CDP do WebView2)', '')
$script:Version = $null

function Add-Summary([string]$Line) { $script:Summary += $Line }

function Get-Cdp([string]$Endpoint) {
    try { return Invoke-RestMethod -Uri "$Cdp/json/$Endpoint" -TimeoutSec 3 } catch { return $null }
}

function Save-Desktop([string]$Target) {
    # Zrzut całego pulpitu (okna Alfy mają WDA_EXCLUDEFROMCAPTURE — na zrzucie systemowym są czarne;
    # treść okien pokazują zrzuty CDP w screens\). Błąd zrzutu nie zmienia wyniku. Tylko Windows
    # (osobna funkcja: PowerShell kompiluje ciało przy pierwszym wywołaniu, a System.Drawing poza
    # Windows rzuca już wtedy).
    if ($PSVersionTable.PSEdition -eq 'Core' -and -not $IsWindows) { return }
    try { Save-DesktopPng $Target } catch { Write-Warning "Zrzut pulpitu nieudany: $($_.Exception.Message)" }
}

function Save-DesktopPng([string]$Target) {
    Add-Type -AssemblyName System.Windows.Forms
    Add-Type -AssemblyName System.Drawing
    $bounds = [System.Windows.Forms.SystemInformation]::VirtualScreen
    $bitmap = New-Object System.Drawing.Bitmap $bounds.Width, $bounds.Height
    $graphics = [System.Drawing.Graphics]::FromImage($bitmap)
    try {
        $graphics.CopyFromScreen($bounds.Left, $bounds.Top, 0, 0, $bitmap.Size)
        $bitmap.Save($Target, [System.Drawing.Imaging.ImageFormat]::Png)
    } finally {
        $graphics.Dispose()
        $bitmap.Dispose()
    }
}

function Get-Prop($Object, [string]$Name) {
    if ($null -eq $Object) { return $null }
    $property = $Object.PSObject.Properties[$Name]
    if ($null -eq $property) { return $null }
    return $property.Value
}

function Add-TestResults([string]$File) {
    if (-not (Test-Path $File)) { Add-Summary '- brak `results.json` (Playwright nie wystartował)'; return }
    $report = Get-Content -Raw -Encoding UTF8 $File | ConvertFrom-Json
    $stats = Get-Prop $report 'stats'
    Add-Summary ("Testy: {0} zaliczone, {1} oblane, {2} pominięte, {3} niestabilne." -f `
        (Get-Prop $stats 'expected'), (Get-Prop $stats 'unexpected'), (Get-Prop $stats 'skipped'), (Get-Prop $stats 'flaky'))
    Add-Summary ''
    $queue = New-Object System.Collections.Queue
    foreach ($suite in @(Get-Prop $report 'suites')) { $queue.Enqueue($suite) }
    while ($queue.Count -gt 0) {
        $suite = $queue.Dequeue()
        foreach ($child in @(Get-Prop $suite 'suites')) { if ($null -ne $child) { $queue.Enqueue($child) } }
        foreach ($spec in @(Get-Prop $suite 'specs')) {
            if ($null -eq $spec) { continue }
            $statuses = @(foreach ($test in @(Get-Prop $spec 'tests')) {
                    foreach ($result in @(Get-Prop $test 'results')) { Get-Prop $result 'status' } })
            $failed = ($statuses -contains 'failed') -or ($statuses -contains 'timedOut') -or ($statuses -contains 'interrupted')
            $mark = if ($failed) { '❌' } elseif ($statuses -contains 'skipped') { '⏭️' } else { '✅' }
            Add-Summary ("- {0} {1} ({2})" -f $mark, (Get-Prop $spec 'title'), ($statuses -join ', '))
        }
    }
}

function Add-Findings([string]$File) {
    if (-not (Test-Path $File)) { return }
    $findings = @(Get-Content -Encoding UTF8 $File | Where-Object { $_.Trim() } | ForEach-Object { $_ | ConvertFrom-Json })
    $groups = @($findings | Group-Object kind | ForEach-Object { "$($_.Name): $($_.Count)" })
    $counts = if ($groups.Count -gt 0) { $groups -join ', ' } else { 'brak' }
    Add-Summary ''
    Add-Summary ('Ustalenia konsoli i CDP (`console.jsonl`): {0}' -f $counts)
    # Bez wpisów sondy Trusted Types (celowe naruszenie; faza `PROBE_PHASE` z alfa.ts).
    $important = @($findings | Where-Object { $_.phase -ne 'sonda Trusted Types' } |
            Where-Object { $_.kind -in @('csp', 'pageerror') -or ($_.kind -eq 'console' -and $_.level -eq 'error') })
    if ($important.Count -gt 0) {
        Add-Summary ''
        Add-Summary '| rodzaj | okno | faza | treść |'
        Add-Summary '|---|---|---|---|'
        foreach ($f in ($important | Select-Object -First 40)) {
            $text = (($f.text -split "`n")[0] -replace '\|', '\|')
            if ($text.Length -gt 200) { $text = $text.Substring(0, 200) + '…' }
            Add-Summary ("| {0} | {1} | {2} | {3} |" -f $f.kind, $f.window, $f.phase, $text)
        }
    }
}

function Invoke-Safely([string]$What, [scriptblock]$Block) {
    try { & $Block } catch { Write-Warning "${What}: $($_.Exception.Message)" }
}

$proc = $null
$exitCode = 1
try {
    if (-not (Test-Path $Exe)) { throw "Brak $Exe — zbuduj powłokę: cargo build --features `"e2e tauri/custom-protocol`"" }
    $Exe = (Resolve-Path $Exe).Path
    $alfaLocal = Join-Path $env:LOCALAPPDATA 'Alfa'
    if ($env:CI -ne 'true' -and (Test-Path $alfaLocal)) {
        throw "Istnieje $alfaLocal — ten test tworzy dane Alfy i przechodzi wprowadzenie; uruchamiaj go tylko na świeżym profilu (runner CI)."
    }
    if ($null -ne (Get-Cdp 'version')) { throw "Port $Port już odpowiada — inna przeglądarka/WebView2 z debugowaniem; zamknij ją." }

    Write-Host "Start: $Exe"
    $proc = Start-Process -FilePath $Exe -WorkingDirectory (Split-Path -Parent $Exe) -PassThru -NoNewWindow `
        -RedirectStandardOutput (Join-Path $OutDir 'alfa-desktop.stdout.log') `
        -RedirectStandardError (Join-Path $OutDir 'alfa-desktop.stderr.log')
    $null = $proc.Handle  # uchwyt zapamiętany — inaczej ExitCode po zakończeniu bywa pusty
    $deadline = (Get-Date).AddSeconds($StartTimeoutSec)
    while ($null -eq $script:Version) {
        if ($proc.HasExited) { throw "alfa-desktop zakończyła się przed otwarciem portu CDP (kod $($proc.ExitCode)) — zobacz alfa-desktop.stderr.log i logs\." }
        if ((Get-Date) -gt $deadline) { throw "Port CDP $Port nie odpowiedział w ciągu $StartTimeoutSec s." }
        Start-Sleep -Milliseconds 500
        $script:Version = Get-Cdp 'version'
    }
    $script:Version | ConvertTo-Json -Depth 5 | Set-Content -Encoding UTF8 (Join-Path $OutDir 'cdp-version.json')
    Write-Host "CDP: $(Get-Prop $script:Version 'Browser') (pid aplikacji $($proc.Id))"
    Add-Summary ("WebView2: ``{0}`` · aplikacja pid {1}" -f (Get-Prop $script:Version 'Browser'), $proc.Id)
    Add-Summary ''

    $env:ALFA_CDP_URL = $Cdp
    $env:ALFA_E2E_OUT = $OutDir
    Push-Location $UiDir
    try {
        & pnpm exec playwright test -c playwright.app.config.ts
        $exitCode = $LASTEXITCODE
    } finally {
        Pop-Location
    }
} catch {
    Write-Host "::error::$($_.Exception.Message)"
    Add-Summary "**Błąd uruchomienia:** $($_.Exception.Message)"
    $exitCode = 1
} finally {
    # Sprzątanie i raport zawsze; błąd jednego kroku nie przerywa pozostałych ani nie zmienia wyniku.
    Invoke-Safely 'lista celów CDP' {
        $targets = Get-Cdp 'list'
        if ($null -ne $targets) { $targets | ConvertTo-Json -Depth 5 | Set-Content -Encoding UTF8 (Join-Path $OutDir 'cdp-targets.json') }
    }
    Save-Desktop (Join-Path $OutDir 'desktop.png')
    Invoke-Safely 'zakończenie aplikacji' {
        if ($null -eq $proc) { return }
        if ($proc.HasExited) {
            Add-Summary ("**Aplikacja zakończyła się przed końcem testów** (kod {0}) — zobacz `alfa-desktop.stderr.log`." -f $proc.ExitCode)
            return
        }
        # Okno główne zamyka się do zasobnika — kończymy całe drzewo (powłoka + procesy WebView2).
        if ($PSVersionTable.PSEdition -eq 'Desktop' -or $IsWindows) { & taskkill.exe /PID $proc.Id /T /F | Out-Host }
        else { Stop-Process -Id $proc.Id -Force -ErrorAction SilentlyContinue }
        $null = $proc.WaitForExit(15000)
    }
    Invoke-Safely 'kopia logów' {
        $logs = Join-Path $env:LOCALAPPDATA 'Alfa\logs'
        if (Test-Path $logs) { Copy-Item -Recurse -Force $logs (Join-Path $OutDir 'logs') }
    }
    Invoke-Safely 'wyniki Playwrighta' { Add-TestResults (Join-Path $OutDir 'results.json') }
    Invoke-Safely 'ustalenia konsoli' { Add-Findings (Join-Path $OutDir 'console.jsonl') }
    Add-Summary ''
    Add-Summary 'Artefakt joba `alfa-app-e2e`: zrzuty `screens/*.png`, raport `report/index.html`, `console.jsonl`, logi `logs/`, stdout/stderr aplikacji, `desktop.png`.'
    if ($env:GITHUB_STEP_SUMMARY) { $script:Summary | Out-File -FilePath $env:GITHUB_STEP_SUMMARY -Append -Encoding utf8 }
    else { $script:Summary | Write-Host }
}
exit $exitCode
