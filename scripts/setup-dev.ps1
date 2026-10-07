<#
.SYNOPSIS
    Alfa: sprawdzenie i przygotowanie Windows 11 do budowy i pierwszego testu (docs/user-guide/11-pierwszy-test-na-pc.md).
.DESCRIPTION
    Bez przełączników TYLKO SPRAWDZA wymagania i wypisuje, czego brakuje i jak to doinstalować (nic nie zmienia).
    -Install: doinstalowuje braki (winget, rustup, npm), o każdy krok pyta osobno; -WithGpuSdk: także Vulkan SDK/CUDA.
    -Build: pnpm install, interfejs, procesy Jądra, powłoka. -Test: vitest, testy crate'ów Windows, testy na żywo.
    -Run: tauri dev (jak cargo tauri dev). -Installer: instalator NSIS jak workflow wydania (bez podpisu, bez aktualizacji).
    -Ci: tylko sprawdzenie dla CI (.github/workflows/rehearsal.yml): nigdy nie pyta i nic nie instaluje ani nie buduje,
    działa też z konta administratora; kod wyjścia 0 = komplet, 1 = brak narzędzi, 2 = błąd skryptu (linia ALFA_SETUP_RESULT).
    Nie czyta ~/.claude, ~/.codex, ciasteczek ani Menedżera poświadczeń; bez telemetrii, bez skryptów z internetu.
    Sam nie podnosi uprawnień (okno UAC pokazuje tylko instalator, który go wymaga). Można uruchamiać wiele razy.
    Plik w UTF-8 z BOM: bez BOM Windows PowerShell 5.1 psuje polskie znaki w napisach.
.EXAMPLE
    powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\setup-dev.ps1 -Install
#>
[CmdletBinding()]
param([switch]$Install, [switch]$WithGpuSdk, [switch]$Build, [switch]$Test, [switch]$Run, [switch]$Installer, [switch]$Ci)

Set-StrictMode -Version 1.0
# Kody wyjścia programów sprawdzamy sami ($LASTEXITCODE). Przy 'Stop' Windows PowerShell 5.1 przerywałby
# skrypt na każdej linii, którą program wypisze na stderr (np. postęp kompilacji cargo).
$ErrorActionPreference = 'Continue'

# Wersje jak w rust-toolchain.toml, package.json i .github/workflows/ci.yml.
$Root = Split-Path -Parent $PSScriptRoot
$UiDir = Join-Path $Root 'apps\desktop\ui'
$ShellDir = Join-Path $Root 'apps\desktop\src-tauri'
$Toolchain = '1.94-x86_64-pc-windows-msvc'
$MinFreeGb = 40
$MaxRootLength = 50
# Crate'y z kodem tylko dla Windows; ich zwykłe testy przechodzą też w CI (windows-latest).
$WindowsCrates = @('platform-windows-impl', 'platform-windows-sys-impl', 'platform-windows-gui-impl',
    'platform-windows-kernel-impl', 'platform-windows-ocr-impl', 'platform-windows-pty-impl',
    'platform-windows-office-impl', 'app-safety')
$script:Checks = @()
$script:Summary = @()
$script:StepOk = $true

# --- Pomocnicze ----------------------------------------------------------------------------------------
function Get-Output([string]$Exe, [string[]]$ArgList = @()) {
    # Uruchamia program i zwraca jego wyjście (stdout i stderr) jako tekst; $null, gdy programu nie ma.
    if (-not (Get-Command $Exe -ErrorAction SilentlyContinue)) { return $null }
    $lines = & $Exe @ArgList 2>&1 | ForEach-Object { "$_" }
    return (($lines -join "`n").Trim())
}

function Confirm-Step([string]$Question) {
    # Domyślnie NIE: krok uruchamia tylko odpowiedź t / tak (albo y / yes). W trybie -Ci nigdy nie pyta.
    if ($Ci) { return $false }
    return ((Read-Host "$Question [t/N]") -match '^\s*(t|tak|y|yes)\s*$')
}

function New-Setup([string]$Command) {
    # Polecenie instalacji: ten sam tekst jest pokazywany i wykonywany.
    return @{ Text = $Command; Block = [scriptblock]::Create($Command) }
}

function New-Winget([string]$Id, [string]$Extra = '') { return (New-Setup ("winget install --id $Id -e --source winget $Extra".Trim())) }

function Add-Check {
    # Stan: OK, BRAK (wymagane), UWAGA (zalecane), INFO (tylko informacja).
    param([string]$Name, [bool]$Ok, [string]$Detail = '', [string]$Fix = '', $Setup = $null, [string]$Missing = 'BRAK', [switch]$Info)
    $state = if ($Info) { 'INFO' } elseif ($Ok) { 'OK' } else { $Missing }
    if ($Setup -and -not $Fix) { $Fix = $Setup.Text }
    $Detail = $Detail -replace '\s*\r?\n\s*', '; '
    $script:Checks += [pscustomobject]@{ Name = $Name; State = $state; Detail = $Detail; Fix = $Fix; Setup = $Setup }
}

function Update-SessionPath {
    # Instalatory dopisują się do PATH w rejestrze, a to okno tego nie widzi: odświeżamy PATH procesu.
    $parts = @([Environment]::GetEnvironmentVariable('Path', 'Machine'), [Environment]::GetEnvironmentVariable('Path', 'User'))
    foreach ($extra in @((Join-Path $env:USERPROFILE '.cargo\bin'), (Join-Path $env:APPDATA 'npm'))) {
        if (Test-Path -LiteralPath $extra) { $parts += $extra }
    }
    $env:Path = (@($parts | Where-Object { $_ }) -join ';')
}

function Get-RegistryValue([string[]]$Keys, [string]$Name) {
    # Pierwsza niepusta wartość z listy kluczy rejestru (tylko odczyt).
    foreach ($key in $Keys) {
        $item = Get-ItemProperty -LiteralPath $key -Name $Name -ErrorAction SilentlyContinue
        if ($item -and $item.$Name -and $item.$Name -ne '0.0.0.0') { return $item.$Name }
    }
    return $null
}

# --- Sprawdzenie wymagań -------------------------------------------------------------------------------
function Test-System {
    $os = [Environment]::OSVersion.Version
    $win11 = ($os.Major -eq 10) -and ($os.Build -ge 22000) -and [Environment]::Is64BitOperatingSystem
    Add-Check 'Windows 11 (64-bit)' $win11 "kompilacja $($os.Build)" 'Alfa działa tylko na Windows 11 x64.'
    Add-Check 'PowerShell' $true "$($PSVersionTable.PSVersion) $($PSVersionTable.PSEdition)" -Info
    try { $freeGb = [math]::Floor((New-Object System.IO.DriveInfo ([System.IO.Path]::GetPathRoot($Root))).AvailableFreeSpace / 1GB) } catch { $freeGb = -1 }
    $freeDetail = if ($freeGb -ge 0) { "$freeGb GB, zalecane co najmniej $MinFreeGb GB" } else { 'nie udało się odczytać' }
    Add-Check 'Wolne miejsce na dysku' ($freeGb -ge $MinFreeGb) $freeDetail -Missing 'UWAGA' `
        -Fix 'Zwolnij miejsce albo sklonuj repozytorium na inny dysk.'
    Add-Check 'Krótka ścieżka repozytorium' ($Root.Length -le $MaxRootLength) $Root -Missing 'UWAGA' `
        -Fix 'Długie ścieżki psują budowę (limit 260 znaków). Sklonuj repozytorium np. do C:\alfa.'
    $winget = Get-Output 'winget' @('--version')
    Add-Check 'winget (Instalator aplikacji)' ([bool]$winget) $winget -Missing 'UWAGA' `
        -Fix 'Sklep Microsoft: zaktualizuj aplikację Instalator aplikacji (App Installer).'
}

function Test-NativeToolchain {
    $git = Get-Output 'git' @('--version')
    Add-Check 'Git' ([bool]$git) $git -Setup (New-Winget 'Git.Git')
    # Visual Studio Build Tools z C++ (MSVC). vswhere znajdzie też pełne Visual Studio 2022/2026.
    $pf86 = if (${env:ProgramFiles(x86)}) { ${env:ProgramFiles(x86)} } else { 'C:\Program Files (x86)' }
    $vswhere = Join-Path $pf86 'Microsoft Visual Studio\Installer\vswhere.exe'
    $vsQuery = @('-latest', '-products', '*', '-requires', 'Microsoft.VisualStudio.Component.VC.Tools.x86.x64', '-property', 'displayName')
    $vs = if (Test-Path -LiteralPath $vswhere) { Get-Output $vswhere $vsQuery } else { $null }
    $vsSetup = New-Winget 'Microsoft.VisualStudio.2022.BuildTools' `
        '--override "--wait --passive --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended"'
    Add-Check 'Visual Studio Build Tools (C++ x64)' ([bool]$vs) $vs -Setup $vsSetup
    # Windows SDK: biblioteki systemu dla linkera (zalecany składnik obciążenia C++, instaluje się razem z nim).
    $kits = Get-RegistryValue @('HKLM:\SOFTWARE\WOW6432Node\Microsoft\Windows Kits\Installed Roots',
        'HKLM:\SOFTWARE\Microsoft\Windows Kits\Installed Roots') 'KitsRoot10'
    $sdk = $null
    if ($kits -and (Test-Path -LiteralPath (Join-Path $kits 'Lib'))) {
        $sdk = Get-ChildItem -LiteralPath (Join-Path $kits 'Lib') -Directory -ErrorAction SilentlyContinue |
            Where-Object { $_.Name -like '10.0.*' -and (Test-Path -LiteralPath (Join-Path $_.FullName 'um\x64\kernel32.lib')) } |
            Sort-Object Name | Select-Object -Last 1 -ExpandProperty Name
    }
    $sdkFix = if ($vs) { 'Visual Studio Installer: Modyfikuj, zaznacz Windows 11 SDK (w obciążeniu C++).' } else { 'Zainstaluje się razem z Visual Studio Build Tools.' }
    $sdkSetup = if ($vs) { $null } else { $vsSetup }
    Add-Check 'Windows SDK' ([bool]$sdk) $sdk -Fix $sdkFix -Setup $sdkSetup
    # SQLCipher z wbudowanym OpenSSL (rusqlite, openssl-src) buduje się skryptami Perla dla Windows.
    $perl = Get-Output 'perl' @('-e', 'print $^O')
    $perlOk = [bool]($perl -match '(?m)^MSWin32\s*$')
    $perlDetail = if ($perl -and -not $perlOk) { "perl zgłasza: $perl; potrzebny MSWin32 (Strawberry Perl przed Perlem z Gita w PATH)" } else { '' }
    Add-Check 'Strawberry Perl (OpenSSL dla szyfrowanej bazy)' $perlOk $perlDetail -Setup (New-Winget 'StrawberryPerl.StrawberryPerl')
    # rustup pisze też na stderr (kolejność linii bywa różna), więc wersję wyłuskujemy wzorcem.
    $rustup = Get-Output 'rustup' @('--version')
    $rustupLine = if ($rustup -match 'rustup [0-9.]+') { $Matches[0] } else { '' }
    Add-Check 'rustup (instalator Rusta)' ([bool]$rustup) $rustupLine -Setup (New-Winget 'Rustlang.Rustup')
    $chains = if ($rustup) { Get-Output 'rustup' @('toolchain', 'list') } else { '' }
    $hasChain = [bool]($chains -match "(?m)^$([regex]::Escape($Toolchain))")
    $components = if ($hasChain) { Get-Output 'rustup' @('component', 'list', '--installed', '--toolchain', $Toolchain) } else { '' }
    $okChain = $hasChain -and ($components -match 'rustfmt') -and ($components -match 'clippy')
    $chainDetail = if ($hasChain) { $Toolchain } else { '' }
    $chainSetup = New-Setup "rustup toolchain install $Toolchain --profile minimal --component rustfmt --component clippy"
    Add-Check 'Rust 1.94 (MSVC) z rustfmt i clippy' $okChain $chainDetail -Setup $chainSetup
}

function Test-WebToolchain {
    $node = Get-Output 'node' @('--version')
    $nodeMajor = 0; $nodeVersion = ''
    if ($node -match '(?m)^(v(\d+)\.\d+\.\d+)') { $nodeVersion = $Matches[1]; $nodeMajor = [int]$Matches[2] }
    $nodeDetail = if ($nodeMajor -gt 22) { "$nodeVersion (CI używa 22; nowsza też powinna działać)" } else { $nodeVersion }
    Add-Check 'Node.js 22' ($nodeMajor -ge 22) $nodeDetail -Setup (New-Winget 'OpenJS.NodeJS.22')
    $pnpmWanted = '10'
    $packageJson = Get-Content -LiteralPath (Join-Path $Root 'package.json') -Raw
    if ($packageJson -match '"packageManager"\s*:\s*"pnpm@([0-9.]+)"') { $pnpmWanted = $Matches[1] }
    # Poza repozytorium pnpm nie przełącza się na wersję z packageManager (nic nie pobiera).
    Push-Location $env:TEMP; $pnpm = Get-Output 'pnpm' @('--version'); Pop-Location
    $pnpmMajor = 0; $pnpmVersion = ''
    if ($pnpm -match '(?m)^((\d+)\.\d+\.\d+)') { $pnpmVersion = $Matches[1]; $pnpmMajor = [int]$Matches[2] }
    Add-Check "pnpm 10 (repozytorium: $pnpmWanted)" ($pnpmMajor -ge 10) $pnpmVersion `
        -Setup (New-Setup "npm install --global pnpm@$pnpmWanted --no-audit --no-fund")
    $wvKey = '{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}'
    $webview = Get-RegistryValue @("HKLM:\SOFTWARE\WOW6432Node\Microsoft\EdgeUpdate\Clients\$wvKey",
        "HKCU:\Software\Microsoft\EdgeUpdate\Clients\$wvKey") 'pv'
    Add-Check 'WebView2 Runtime' ([bool]$webview) $webview -Setup (New-Winget 'Microsoft.EdgeWebView2Runtime')
    # tauri-cli 2 przychodzi z pnpm (@tauri-apps/cli, wersja przypięta w apps/desktop/ui/package.json).
    $cliPackage = Join-Path $UiDir 'node_modules\@tauri-apps\cli\package.json'
    if (Test-Path -LiteralPath $cliPackage) {
        $cliVersion = (Get-Content -LiteralPath $cliPackage -Raw | ConvertFrom-Json).version
        Add-Check 'tauri-cli 2 (@tauri-apps/cli z pnpm)' ($cliVersion -like '2.*') $cliVersion
    } else {
        Add-Check 'tauri-cli 2 (@tauri-apps/cli z pnpm)' $false 'jeszcze nie ma' -Missing 'UWAGA' -Fix 'Zainstaluje go krok -Build (pnpm install).'
    }
    $cargoTauri = Get-Output 'cargo-tauri' @('--version')
    if ($cargoTauri) { Add-Check 'cargo tauri (opcjonalnie)' $true $cargoTauri -Info }
}

function Test-Gpu {
    $gpus = @(Get-CimInstance -ClassName Win32_VideoController -ErrorAction SilentlyContinue)
    foreach ($gpu in $gpus) { Add-Check 'Karta graficzna' $true "$($gpu.Name), sterownik $($gpu.DriverVersion)" -Info }
    $vulkan = Test-Path -LiteralPath (Join-Path $env:SystemRoot 'System32\vulkan-1.dll')
    Add-Check 'Vulkan (instaluje go sterownik karty)' $vulkan '' -Missing 'UWAGA' -Fix 'Zainstaluj aktualny sterownik karty ze strony AMD albo NVIDIA.'
    $nvidia = @($gpus | Where-Object { $_.Name -match 'NVIDIA' }).Count -gt 0
    if ($nvidia) {
        $smi = Get-Output 'nvidia-smi' @('--query-gpu=name,driver_version', '--format=csv,noheader')
        Add-Check 'Sterownik NVIDIA (nvidia-smi)' ([bool]$smi) $smi -Missing 'UWAGA' -Fix 'Zainstaluj sterownik Game Ready albo Studio ze strony NVIDIA.'
    }
    # SDK tylko do samodzielnej kompilacji whisper.cpp / llama.cpp (spike'i F0); Alfa używa gotowych programów.
    $absent = 'brak; do testu Alfy niepotrzebny'
    $vkSetup = if ($WithGpuSdk) { New-Winget 'KhronosGroup.VulkanSDK' } else { $null }
    $vkDetail = if ($env:VULKAN_SDK) { $env:VULKAN_SDK } else { $absent }
    Add-Check 'Vulkan SDK (opcjonalnie)' ([bool]$env:VULKAN_SDK) $vkDetail -Setup $vkSetup -Missing 'INFO'
    if ($nvidia) {
        $cudaSetup = if ($WithGpuSdk) { New-Winget 'Nvidia.CUDA' } else { $null }
        $cudaDetail = if ($env:CUDA_PATH) { $env:CUDA_PATH } else { $absent }
        Add-Check 'CUDA Toolkit (opcjonalnie)' ([bool]$env:CUDA_PATH) $cudaDetail -Setup $cudaSetup -Missing 'INFO'
    }
}

function Test-Requirements { $script:Checks = @(); Test-System; Test-NativeToolchain; Test-WebToolchain; Test-Gpu }

function Show-Checks {
    Write-Host "`nWymagania do budowy Alfy ($Root)" -ForegroundColor Cyan
    foreach ($c in $script:Checks) {
        $color = switch ($c.State) { 'OK' { 'Green' } 'BRAK' { 'Red' } 'UWAGA' { 'Yellow' } default { 'Gray' } }
        Write-Host ('  {0,-8}' -f "[$($c.State)]") -ForegroundColor $color -NoNewline
        Write-Host $c.Name -NoNewline
        if ($c.Detail) { Write-Host "  ($($c.Detail))" -ForegroundColor DarkGray } else { Write-Host '' }
        if (($c.State -eq 'BRAK' -or $c.State -eq 'UWAGA') -and $c.Fix) { Write-Host "          co zrobić: $($c.Fix)" -ForegroundColor DarkYellow }
    }
}

# --- Instalacja (tylko z -Install, każdy krok po zgodzie) --------------------------------------------------
function Invoke-Install {
    $todo = @($script:Checks | Where-Object { $_.State -ne 'OK' -and $_.Setup })
    Write-Host ''
    if ($todo.Count -eq 0) { Write-Host 'Nie ma nic do zainstalowania.' -ForegroundColor Green; return }
    Write-Host 'Instalacja brakujących narzędzi. Każdy krok wymaga Twojej zgody (t = tak, sam Enter = nie).' -ForegroundColor Cyan
    Write-Host 'Instalatory Visual Studio, Node.js, Perla i Gita same poproszą o zgodę administratora (okno UAC).'
    $done = @{}
    foreach ($c in $todo) {
        if ($done.ContainsKey($c.Setup.Text)) { continue }
        $done[$c.Setup.Text] = $true
        Write-Host "`nBrakuje: $($c.Name)" -ForegroundColor Yellow
        Write-Host "  polecenie: $($c.Setup.Text)"
        if ($c.Setup.Text -like 'winget *' -and -not (Get-Command 'winget' -ErrorAction SilentlyContinue)) {
            Write-Host '  Brak winget: najpierw zaktualizuj Instalator aplikacji (App Installer) w Sklepie Microsoft.' -ForegroundColor Red
            continue
        }
        if (-not (Confirm-Step '  Uruchomić?')) { Write-Host '  Pominięto.'; continue }
        $global:LASTEXITCODE = 0
        try { & $c.Setup.Block } catch { Write-Host "  Błąd: $($_.Exception.Message)" -ForegroundColor Red; $global:LASTEXITCODE = 1 }
        if ($LASTEXITCODE -eq 3010) { Write-Host '  Zainstalowano; Windows wymaga ponownego uruchomienia komputera.' -ForegroundColor Yellow }
        elseif ($LASTEXITCODE -ne 0) { Write-Host "  Instalator zwrócił kod $LASTEXITCODE. Przeczytaj komunikat powyżej." -ForegroundColor Red }
        Update-SessionPath
    }
}

# --- Budowa, testy, instalator, uruchomienie ----------------------------------------------------------------
function Invoke-Step([string]$Title, [scriptblock]$Action) {
    # Uruchamia krok; wynik trafia do podsumowania, a $script:StepOk mówi, czy krok się udał.
    Write-Host "`n==> $Title" -ForegroundColor Cyan
    $started = Get-Date
    $global:LASTEXITCODE = 0
    try { & $Action } catch { Write-Host "Błąd: $($_.Exception.Message)" -ForegroundColor Red; $global:LASTEXITCODE = 1 }
    $code = $LASTEXITCODE; $script:StepOk = ($code -eq 0)
    $result = if ($script:StepOk) { 'OK' } else { "BŁĄD (kod $code)" }
    $minutes = [math]::Round(((Get-Date) - $started).TotalMinutes, 1)
    $script:Summary += [pscustomobject]@{ Krok = $Title; Wynik = $result; Minuty = $minutes }
    $color = if ($script:StepOk) { 'Green' } else { 'Red' }
    Write-Host "<== $result, $minutes min: $Title" -ForegroundColor $color
}

function Invoke-Build {
    $shellManifest = Join-Path $ShellDir 'Cargo.toml'
    Invoke-Step 'Zależności interfejsu: pnpm install --frozen-lockfile' { pnpm install --frozen-lockfile }
    if ($script:StepOk) { Invoke-Step 'Interfejs: pnpm --filter @alfa/desktop-ui build' { pnpm --filter '@alfa/desktop-ui' build } }
    if ($script:StepOk) { Invoke-Step 'Procesy Jądra (Broker, okno zatwierdzeń, watchdog): cargo build -p app-safety' { cargo build --locked -p app-safety --bins } }
    if ($script:StepOk) { Invoke-Step 'Powłoka Alfy: cargo build (apps\desktop\src-tauri)' { cargo build --manifest-path $shellManifest } }
}

function New-LiveTest([string]$Name, [string]$Crate, [string]$Bin, [string]$Filter, [string]$Note) {
    $testArgs = @('-p', $Crate, '--test', $Bin, '--', '--ignored')
    if ($Filter) { $testArgs += $Filter }
    return [pscustomobject]@{ Name = $Name; Args = $testArgs; Note = $Note }
}

function Invoke-Tests {
    Invoke-Step 'Testy interfejsu (vitest)' { pnpm --filter '@alfa/desktop-ui' test }
    $packages = @()
    foreach ($crate in $WindowsCrates) { $packages += @('-p', $crate) }
    Invoke-Step "Testy crate'ów Windows (cargo test, $($WindowsCrates.Count) crate'ów)" { cargo test @packages }
    # Testy #[ignore] na żywym systemie (w CI robi to runner self-hosted). Celowo pominięte: accounts-hub-impl/credman
    # (zapis do Menedżera poświadczeń), app-safety/windows_ports (usługa LocalSystem), Word i Edge w office (plik
    # ALFA_LIVE_DOCX, ruch sieciowy), modele głosu (ALFA_*_MODEL), app-agents/eval_tools (pomiar F3-07, evals/F3/tools).
    $live = @(
        (New-LiveTest 'system: procesy, usługi, dziennik zdarzeń, zmienne' 'platform-windows-sys-impl' 'live_sys' '' 'Głównie odczyt; zmienną testową ZMIENNA_TESTOWA_ALFY_LIVE ustawia i od razu przywraca.'),
        (New-LiveTest 'Kosz, skróty globalne i schowek' 'platform-windows-impl' 'windows_system' '' 'NADPISZE SCHOWEK (skopiuj wcześniej ważną treść). Na chwilę rejestruje Ctrl+Shift+F12: Alfa musi być zamknięta.'),
        (New-LiveTest 'Notatnik: drzewo UIA, wpisywanie tekstu, zrzut okna' 'platform-windows-gui-impl' 'gui_windows' '' 'Zamknij wcześniej wszystkie okna Notatnika (test zamyka go siłą). Przez ok. 30 s nie ruszaj myszą ani klawiaturą.'),
        (New-LiveTest 'okno zatwierdzeń: 100 kliknięć programowych (F0-16)' 'platform-windows-kernel-impl' 'kernel_windows' '' 'Kursor sam kliknie 100 razy w testowe okno zatwierdzeń. Nie dotykaj myszy (ok. 1 min).'),
        (New-LiveTest 'rejestr Windows (odczyt, odmowa sekretów)' 'platform-windows-office-impl' 'live_windows' 'registry_reads_and_denies_secrets' 'Tylko odczyt rejestru.'),
        (New-LiveTest 'mikrofon i głośniki (WASAPI)' 'voice-audio-impl' 'module' 'contract_suite_on_hardware' 'Usłyszysz kilka krótkich tonów; mikrofon jest otwierany na chwilę, nic nie trafia na dysk.'),
        (New-LiveTest 'blokada ekranu (Win+L)' 'platform-windows-sys-impl' 'sys_windows' 'manual_lock_is_reported' 'Po napisie running 1 test masz 20 s: naciśnij Win+L, potem odblokuj komputer.'),
        (New-LiveTest 'tryb gry (pełny ekran)' 'platform-windows-sys-impl' 'sys_windows' 'manual_fullscreen_is_game_mode' 'Po napisie running 1 test masz 30 s: włącz film na pełnym ekranie (np. wideo w przeglądarce i F11).')
    )
    Write-Host "`nTesty na żywym systemie: każdy uruchamiasz osobno. Zamknij wcześniej Alfę (także ikonę w zasobniku)." -ForegroundColor Cyan
    foreach ($liveTest in $live) {
        Write-Host "`nTest na żywo: $($liveTest.Name)" -ForegroundColor Cyan
        Write-Host "  $($liveTest.Note)" -ForegroundColor Yellow
        if (Confirm-Step '  Uruchomić?') {
            $testArgs = $liveTest.Args
            Invoke-Step "Na żywo: $($liveTest.Name)" { cargo test @testArgs }
        } else {
            $script:Summary += [pscustomobject]@{ Krok = "Na żywo: $($liveTest.Name)"; Wynik = 'pominięty'; Minuty = 0 }
        }
    }
}

function Invoke-InstallerBuild {
    # Te same kroki co .github/workflows/release.yml (bez licencji, podpisu minisign i adresu aktualizacji).
    $triple = 'x86_64-pc-windows-msvc'
    $binaries = Join-Path $ShellDir 'binaries'
    $release = Join-Path $Root 'target\release'
    Invoke-Step 'Launcher alfa.exe (release): cargo build -p updater-impl --bin alfa' { cargo build --release --locked -p updater-impl --bin alfa }
    if ($script:StepOk) { Invoke-Step 'Procesy Jądra (release): cargo build -p app-safety --bins' { cargo build --release --locked -p app-safety --bins } }
    if (-not $script:StepOk) { return }
    # Nakładka tauri.bundle.conf.json oczekuje binarek z sufiksem platformy w src-tauri\binaries (poza gitem).
    $copies = [ordered]@{ 'alfa' = 'alfa-launcher'; 'alfa-broker' = 'alfa-broker'; 'alfa-broker-ui' = 'alfa-broker-ui'; 'alfa-watchdog' = 'alfa-watchdog' }
    Invoke-Step 'Binarki dla instalatora: kopiowanie do apps\desktop\src-tauri\binaries' {
        New-Item -ItemType Directory -Force -Path $binaries | Out-Null
        foreach ($name in $copies.Keys) {
            Copy-Item -LiteralPath (Join-Path $release "$name.exe") -Destination (Join-Path $binaries "$($copies[$name])-$triple.exe") -Force
            if (-not $?) { $global:LASTEXITCODE = 1; return }
        }
    }
    if (-not $script:StepOk) { return }
    $overlay = Join-Path $ShellDir 'tauri.bundle.conf.json'
    $env:TAURI_APP_PATH = $ShellDir
    Invoke-Step 'Instalator NSIS: tauri build z nakładką tauri.bundle.conf.json' { pnpm --dir $UiDir exec tauri build --config $overlay }
    $setup = Get-ChildItem -Path (Join-Path $ShellDir 'target\release\bundle\nsis') -Filter '*-setup.exe' -ErrorAction SilentlyContinue | Select-Object -First 1
    if ($script:StepOk -and $setup) { Write-Host "Instalator: $($setup.FullName)" -ForegroundColor Green }
}

function Invoke-Run {
    Write-Host "`nUruchamiam Alfę w trybie deweloperskim (tauri dev). Pierwszy start kompiluje powłokę: kilka minut." -ForegroundColor Cyan
    Write-Host 'Zamknięcie okna chowa Alfę do zasobnika. Całkiem wyłączysz ją: ikona w zasobniku, Wyjście (albo Ctrl+C tutaj).'
    # Tauri CLI szuka src-tauri tylko w głąb katalogu bieżącego (tu: apps\desktop\ui), stąd TAURI_APP_PATH.
    $env:TAURI_APP_PATH = $ShellDir
    pnpm --dir $UiDir exec tauri dev
}

function Write-CiReport([string]$Result) {
    # -Ci: wynik maszynowy (ostatnia linia) i tabela w podsumowaniu joba GitHub Actions (UTF-8 bez BOM).
    Write-Host "ALFA_SETUP_RESULT=$Result"; if (-not $env:GITHUB_STEP_SUMMARY) { return }
    $rows = @("### setup-dev.ps1 -Ci: PowerShell $($PSVersionTable.PSVersion) $($PSVersionTable.PSEdition), wynik: $Result", '', '| Stan | Sprawdzenie | Szczegóły |', '| --- | --- | --- |')
    foreach ($c in $script:Checks) { $rows += "| $($c.State) | $($c.Name) | $($c.Detail -replace '\|', '/') |" }
    [System.IO.File]::AppendAllText($env:GITHUB_STEP_SUMMARY, (($rows + '') -join "`n") + "`n", (New-Object System.Text.UTF8Encoding $false))
}

function Show-Summary {
    if ($script:Summary.Count -eq 0) { return }
    Write-Host "`nPodsumowanie" -ForegroundColor Cyan
    foreach ($row in $script:Summary) {
        $color = if ($row.Wynik -eq 'OK') { 'Green' } elseif ($row.Wynik -like 'BŁĄD*') { 'Red' } else { 'Gray' }
        Write-Host ('  {0,-16} {1,6} min  {2}' -f $row.Wynik, $row.Minuty, $row.Krok) -ForegroundColor $color
    }
}

# --- Start ------------------------------------------------------------------------------------------------
if ($env:OS -ne 'Windows_NT') { Write-Host 'Ten skrypt jest przeznaczony dla Windows 11.' -ForegroundColor Red; exit 1 }
$principal = New-Object Security.Principal.WindowsPrincipal ([Security.Principal.WindowsIdentity]::GetCurrent())
if ($principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator) -and -not $Ci) {
    Write-Host 'Uruchom skrypt w zwykłym oknie Terminala (bez Uruchom jako administrator); instalatory same poproszą o zgodę.' -ForegroundColor Red; exit 1
}
if (-not (Test-Path -LiteralPath (Join-Path $Root 'Cargo.toml'))) { Write-Host "Nie znaleziono repozytorium Alfy w $Root." -ForegroundColor Red; exit 1 }

if ($Ci) { $Install = $false; $Build = $false; $Test = $false; $Run = $false; $Installer = $false } # -Ci: tylko sprawdzenie
$mode = if ($Install) { 'sprawdzenie i instalacja (każdy krok po Twojej zgodzie)' } else { 'tylko sprawdzenie (niczego nie zmieniam)' }
Write-Host "Alfa: przygotowanie komputera. Tryb: $mode." -ForegroundColor Cyan
$previousAutoInstall = $env:RUSTUP_AUTO_INSTALL
# Samo sprawdzanie nie może niczego instalować: rustup doinstalowałby toolchain z rust-toolchain.toml.
$env:RUSTUP_AUTO_INSTALL = '0'
$exitCode = 0
$actions = $Build -or $Test -or $Installer -or $Run
Push-Location $Root
try {
    Test-Requirements
    Show-Checks
    if ($Install) { Invoke-Install; Test-Requirements; Show-Checks }
    $missing = @($script:Checks | Where-Object { $_.State -eq 'BRAK' })
    Write-Host ''
    if ($missing.Count -gt 0) {
        $exitCode = 1
        Write-Host "Brakuje wymaganych elementów: $($missing.Count)." -ForegroundColor Red
        $hint = if ($Install) { 'Zamknij to okno, otwórz nowe i sprawdź jeszcze raz (po Visual Studio czasem potrzebny jest restart).' }
        else { 'Doinstaluj je: powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\setup-dev.ps1 -Install' }
        Write-Host $hint
        if ($actions) { Write-Host 'Budowa, testy i uruchomienie wymagają kompletu narzędzi: pomijam je.' -ForegroundColor Red }
    } else {
        Write-Host 'Wszystkie wymagane narzędzia są gotowe.' -ForegroundColor Green
    }
    if ($actions -and $missing.Count -eq 0) {
        if ($Build) { Invoke-Build }
        $ready = $script:StepOk -and (Test-Path -LiteralPath (Join-Path $UiDir 'node_modules'))
        if (-not $ready -and -not $Build) { Write-Host 'Brak zależności interfejsu: najpierw uruchom skrypt z -Build.' -ForegroundColor Red }
        if ($ready -and $Test) { Invoke-Tests }
        if ($ready -and $Installer) { Invoke-InstallerBuild }
        Show-Summary
        if (-not $ready -or @($script:Summary | Where-Object { $_.Wynik -like 'BŁĄD*' }).Count -gt 0) { $exitCode = 1 }
        if ($ready -and $Run) { Invoke-Run }
    }
} catch {
    $exitCode = 2
    Write-Host "Nieoczekiwany błąd skryptu (linia $($_.InvocationInfo.ScriptLineNumber)): $($_.Exception.Message)" -ForegroundColor Red
    Write-Host 'Skopiuj ten komunikat i zgłoś go (docs/user-guide/11-pierwszy-test-na-pc.md, część 13).'
} finally {
    Pop-Location
    $env:RUSTUP_AUTO_INSTALL = $previousAutoInstall
}
if ($Ci) { Write-CiReport (@('ok', 'missing', 'error')[$exitCode]) }
exit $exitCode
