<#
.SYNOPSIS
    Alfa: całe środowisko do budowy i testu na wybranym dysku (domyślnie D:) — jedno polecenie na świeżym komputerze.
.DESCRIPTION
    Na dysku <litera>: zakłada dwa katalogi z dostępem tylko dla Ciebie, SYSTEM i Administratorów:
      <litera>:\alfa            repozytorium Alfy (kod i wyniki kompilacji — największa część, kilkadziesiąt GB),
      <litera>:\alfa-narzedzia  Rust (RUSTUP_HOME, CARGO_HOME), Visual Studio Build Tools (instalacja, pamięć podręczna,
                                składniki wspólne), Git, Node.js 22, Strawberry Perl, pamięć podręczna npm, magazyn pnpm.
    Potem uruchamia scripts\setup-dev.ps1 (sprawdzenie i -Build) i zakłada na Pulpicie skrót do Alfy.
    Na C: zostają: Windows SDK i Instalator Visual Studio (Microsoft nie pozwala ich przenieść), WebView2 Runtime oraz dane
    Alfy (%LOCALAPPDATA%\Alfa: modele, sesje, logi). Danych Alfy nie przenosimy dowiązaniem: Broker chroni pliki Alfy według
    ścieżki, a ten sam plik widziany jako <litera>:\… wypadłby spod tej ochrony (przegląd 2026-10, SR-07).
    Domyślnie pyta o zgodę przed każdym krokiem; -Yes: bez pytań (okna UAC instalatorów i tak się pokażą).
    -NoBuild: bez kompilacji; -Run: na końcu uruchamia Alfę. -Ci: tylko plan i sprawdzenie warunków, niczego nie zmienia
    (.github/workflows/rehearsal.yml); kod 0 = gotowe, 1 = warunek niespełniony, 2 = błąd skryptu (linia ALFA_INSTALL_RESULT).
    Można uruchamiać wiele razy: kroki już wykonane są pomijane. Nie czyta ~/.claude, ~/.codex ani Menedżera poświadczeń.
    Plik w UTF-8 z BOM: bez BOM Windows PowerShell 5.1 psuje polskie znaki w napisach.
.EXAMPLE
    powershell -NoProfile -ExecutionPolicy Bypass -File .\install-on-drive.ps1 -Drive D -Yes
#>
[CmdletBinding()]
param([string]$Drive = 'D', [switch]$Yes, [switch]$NoBuild, [switch]$Run, [switch]$Ci)

Set-StrictMode -Version 1.0
# Kody wyjścia programów sprawdzamy sami ($LASTEXITCODE) — jak w setup-dev.ps1.
$ErrorActionPreference = 'Continue'

$RepoUrl = 'https://github.com/arkadiuszpopiel-eng/program-do-czenia-wielu.git'
$Toolchain = '1.94-x86_64-pc-windows-msvc'
$RustupUrl = 'https://static.rust-lang.org/rustup/dist/x86_64-pc-windows-msvc/rustup-init.exe'
$MinFreeGb = 60
$MinSystemFreeGb = 15
$Letter = $Drive.Trim().TrimEnd('\').TrimEnd(':').ToUpperInvariant()
$RepoDir = "${Letter}:\alfa"
$ToolsDir = "${Letter}:\alfa-narzedzia"
$RustDirs = [ordered]@{ RUSTUP_HOME = (Join-Path $ToolsDir 'rustup'); CARGO_HOME = (Join-Path $ToolsDir 'cargo') }
$VsRoot = Join-Path $ToolsDir 'VisualStudio'
$script:Results = @()
$script:Failed = 0
$script:Reboot = $false

# --- Pomocnicze ----------------------------------------------------------------------------------------
function Get-Output([string]$Exe, [string[]]$ArgList = @()) {
    if (-not (Get-Command $Exe -ErrorAction SilentlyContinue)) { return '' }
    $lines = & $Exe @ArgList 2>&1 | ForEach-Object { "$_" }
    return (($lines -join "`n").Trim())
}

function Confirm-Step([string]$Question) {
    # Domyślnie NIE; -Yes: zawsze tak; -Ci: nigdy (tylko plan).
    if ($Ci) { return $false }
    if ($Yes) { return $true }
    return ((Read-Host "$Question [t/N]") -match '^\s*(t|tak|y|yes)\s*$')
}

function Add-Result([string]$Step, [string]$State, [string]$Detail = '') {
    $script:Results += [pscustomobject]@{ Step = $Step; State = $State; Detail = $Detail }
    if ($State -like 'BŁĄD*') { $script:Failed++ }
    $color = switch -Wildcard ($State) { 'OK' { 'Green' } 'BŁĄD*' { 'Red' } 'UWAGA' { 'Yellow' } default { 'Gray' } }
    Write-Host ('  {0,-12} {1}' -f "[$State]", $Step) -ForegroundColor $color -NoNewline
    if ($Detail) { Write-Host "  ($Detail)" -ForegroundColor DarkGray } else { Write-Host '' }
}

function Update-SessionPath {
    # Instalatory dopisują się do PATH w rejestrze; to okno tego nie widzi.
    $parts = @([Environment]::GetEnvironmentVariable('Path', 'Machine'), [Environment]::GetEnvironmentVariable('Path', 'User'))
    $env:Path = (@($parts | Where-Object { $_ }) -join ';')
}

function Set-OwnerOnlyAcl([string]$Path) {
    # Dysk inny niż systemowy zwykle daje „Użytkownikom uwierzytelnionym” prawo zmiany — kompilator i kod Alfy
    # mogłoby wtedy podmienić inne konto. Zostawiamy: Ty, SYSTEM (S-1-5-18), Administratorzy (S-1-5-32-544).
    $sid = [Security.Principal.WindowsIdentity]::GetCurrent().User.Value
    $null = & icacls $Path /inheritance:r /grant:r "*${sid}:(OI)(CI)F" '*S-1-5-18:(OI)(CI)F' '*S-1-5-32-544:(OI)(CI)F' 2>&1
    return ($LASTEXITCODE -eq 0)
}

function New-OwnedDir([string]$Path, [string]$Name) {
    if (Test-Path -LiteralPath $Path) { Add-Result $Name 'OK' "$Path już jest"; return $true }
    if ($Ci) { Add-Result $Name 'PLAN' "utworzyć $Path (dostęp: Ty, SYSTEM, Administratorzy)"; return $true }
    if (-not (Confirm-Step "Utworzyć $Path?")) { Add-Result $Name 'pominięty'; return $false }
    New-Item -ItemType Directory -Force -Path $Path | Out-Null
    if (Set-OwnerOnlyAcl $Path) { Add-Result $Name 'OK' $Path; return $true }
    Add-Result $Name 'BŁĄD' "icacls nie ustawił uprawnień $Path"; return $false
}

# --- Warunki -------------------------------------------------------------------------------------------
function Test-Preconditions {
    $ok = $true
    $os = [Environment]::OSVersion.Version
    $win11 = ($os.Major -eq 10) -and ($os.Build -ge 22000) -and [Environment]::Is64BitOperatingSystem
    if ($win11) { Add-Result 'Windows 11 x64' 'OK' "kompilacja $($os.Build)" } else { Add-Result 'Windows 11 x64' 'BŁĄD' "kompilacja $($os.Build)"; $ok = $false }
    if ($Letter -notmatch '^[A-Z]$') { Add-Result 'Dysk docelowy' 'BŁĄD' "niepoprawna litera: '$Drive'"; return $false }
    $info = $null
    try { $info = New-Object System.IO.DriveInfo "${Letter}:\" } catch { $info = $null }
    if (-not $info -or -not $info.IsReady) { Add-Result "Dysk ${Letter}:" 'BŁĄD' 'nie istnieje albo nie jest gotowy'; return $false }
    $kind = "$($info.DriveType), $($info.DriveFormat)"
    if ("$($info.DriveType)" -ne 'Fixed' -or $info.DriveFormat -ne 'NTFS') {
        Add-Result "Dysk ${Letter}:" 'BŁĄD' "$kind — potrzebny dysk wewnętrzny NTFS (uprawnienia, dowiązania pnpm)"; $ok = $false
    } else { Add-Result "Dysk ${Letter}:" 'OK' $kind }
    $free = [math]::Floor($info.AvailableFreeSpace / 1GB)
    $state = if ($free -ge $MinFreeGb) { 'OK' } else { 'UWAGA' }
    Add-Result "Wolne miejsce na ${Letter}:" $state "$free GB, zalecane co najmniej $MinFreeGb GB"
    try {
        $sys = New-Object System.IO.DriveInfo ($env:SystemDrive + '\')
        $sysFree = [math]::Floor($sys.AvailableFreeSpace / 1GB)
        $state = if ($sysFree -ge $MinSystemFreeGb) { 'OK' } else { 'UWAGA' }
        Add-Result "Wolne miejsce na $($env:SystemDrive)" $state "$sysFree GB; Windows SDK, Instalator VS i modele Alfy (ok. 8 GB) — zalecane $MinSystemFreeGb GB"
    } catch { Add-Result "Wolne miejsce na $($env:SystemDrive)" 'UWAGA' 'nie udało się odczytać' }
    $winget = Get-Output 'winget' @('--version')
    if ($winget) { Add-Result 'winget (Instalator aplikacji)' 'OK' $winget }
    else { Add-Result 'winget (Instalator aplikacji)' 'UWAGA' 'brak — zaktualizuj Instalator aplikacji (App Installer) w Sklepie Microsoft' }
    return $ok
}

# --- Programy z winget -----------------------------------------------------------------------------------
function Get-VsWithCpp {
    $pf86 = if (${env:ProgramFiles(x86)}) { ${env:ProgramFiles(x86)} } else { 'C:\Program Files (x86)' }
    $vswhere = Join-Path $pf86 'Microsoft Visual Studio\Installer\vswhere.exe'
    if (-not (Test-Path -LiteralPath $vswhere)) { return '' }
    return (Get-Output $vswhere @('-latest', '-products', '*', '-requires', 'Microsoft.VisualStudio.Component.VC.Tools.x86.x64', '-property', 'installationPath'))
}

function Test-WebView2 {
    $key = '{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}'
    foreach ($path in @("HKLM:\SOFTWARE\WOW6432Node\Microsoft\EdgeUpdate\Clients\$key", "HKCU:\Software\Microsoft\EdgeUpdate\Clients\$key")) {
        $item = Get-ItemProperty -LiteralPath $path -Name 'pv' -ErrorAction SilentlyContinue
        if ($item -and $item.pv -and $item.pv -ne '0.0.0.0') { return $true }
    }
    return $false
}

$VsOverride = "--wait --passive --norestart --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended " +
    "--installPath $VsRoot\BuildTools --path cache=$VsRoot\cache --path shared=$VsRoot\shared"
$Packages = @(
    [pscustomobject]@{ Name = 'Git'; Id = 'Git.Git'; Location = 'Git'; Override = ''; Present = { [bool](Get-Output 'git' @('--version')) } },
    [pscustomobject]@{ Name = 'Visual Studio Build Tools (C++ x64, Windows SDK)'; Id = 'Microsoft.VisualStudio.2022.BuildTools'; Location = ''
        Override = $VsOverride; Present = { [bool](Get-VsWithCpp) } },
    [pscustomobject]@{ Name = 'Strawberry Perl (OpenSSL dla szyfrowanej bazy)'; Id = 'StrawberryPerl.StrawberryPerl'; Location = 'StrawberryPerl'
        Override = ''; Present = { (Get-Output 'perl' @('-e', 'print $^O')) -match '(?m)^MSWin32\s*$' } },
    [pscustomobject]@{ Name = 'Node.js 22'; Id = 'OpenJS.NodeJS.22'; Location = 'nodejs'; Override = ''
        Present = { (Get-Output 'node' @('--version')) -match '^v(2[2-9]|[3-9]\d)\.' } },
    [pscustomobject]@{ Name = 'WebView2 Runtime'; Id = 'Microsoft.EdgeWebView2Runtime'; Location = ''; Override = ''; Present = { Test-WebView2 } }
)

function Install-Package($Pkg) {
    if (& $Pkg.Present) { Add-Result $Pkg.Name 'OK' 'już zainstalowany'; return }
    $where = if ($Pkg.Override) { "do $VsRoot" } elseif ($Pkg.Location) { 'do ' + (Join-Path $ToolsDir $Pkg.Location) } else { 'na dysk systemowy' }
    if ($Ci) { Add-Result $Pkg.Name 'PLAN' "winget install --id $($Pkg.Id) $where"; return }
    if (-not (Get-Command 'winget' -ErrorAction SilentlyContinue)) { Add-Result $Pkg.Name 'BŁĄD' 'brak winget'; return }
    if (-not (Confirm-Step "Zainstalować: $($Pkg.Name) ($where)?")) { Add-Result $Pkg.Name 'pominięty'; return }
    $wgArgs = @('install', '--id', $Pkg.Id, '-e', '--source', 'winget', '--accept-package-agreements', '--accept-source-agreements')
    if ($Pkg.Override) { $wgArgs += @('--override', $Pkg.Override) }
    elseif ($Pkg.Location) { $wgArgs += @('--location', (Join-Path $ToolsDir $Pkg.Location)) }
    Write-Host "  winget $($wgArgs -join ' ')" -ForegroundColor DarkGray
    & winget @wgArgs
    $code = $LASTEXITCODE
    Update-SessionPath
    if ($code -eq 3010) { $script:Reboot = $true }
    if (& $Pkg.Present) { Add-Result $Pkg.Name 'OK' "zainstalowany $where" }
    elseif ($code -eq 3010) { Add-Result $Pkg.Name 'UWAGA' 'zainstalowany; potrzebny restart komputera' }
    else { Add-Result $Pkg.Name "BŁĄD (kod $code)" 'przeczytaj komunikat instalatora powyżej' }
}

# --- Rust -----------------------------------------------------------------------------------------------
function Set-UserVariable([string]$Name, [string]$Value) {
    if ([Environment]::GetEnvironmentVariable($Name, 'User') -eq $Value) { Set-Item -Path "Env:$Name" -Value $Value; return $true }
    if ($Ci) { Add-Result "Zmienna $Name" 'PLAN' $Value; return $true }
    if (-not (Confirm-Step "Ustawić zmienną użytkownika $Name = $Value?")) { Add-Result "Zmienna $Name" 'pominięty'; return $false }
    [Environment]::SetEnvironmentVariable($Name, $Value, 'User')
    Set-Item -Path "Env:$Name" -Value $Value
    Add-Result "Zmienna $Name" 'OK' $Value
    return $true
}

function Install-Rustup {
    if ($Ci) { Add-Result 'rustup' 'PLAN' "rustup-init.exe z static.rust-lang.org (SHA-256) do $($RustDirs.CARGO_HOME)"; return }
    # Bez MSVC rustup-init z -y sam pobrałby pełne Visual Studio Community na C:.
    if (-not (Get-VsWithCpp)) { Add-Result 'rustup' 'BŁĄD' 'najpierw Visual Studio Build Tools (C++)'; return }
    if (-not (Confirm-Step "Zainstalować rustup do $($RustDirs.CARGO_HOME)?")) { Add-Result 'rustup' 'pominięty'; return }
    $dir = Join-Path $ToolsDir 'pobrane'
    New-Item -ItemType Directory -Force -Path $dir | Out-Null
    $exe = Join-Path $dir 'rustup-init.exe'
    $previous = $ProgressPreference; $ProgressPreference = 'SilentlyContinue'
    [Net.ServicePointManager]::SecurityProtocol = [Net.ServicePointManager]::SecurityProtocol -bor [Net.SecurityProtocolType]::Tls12
    try {
        Invoke-WebRequest -UseBasicParsing -Uri $RustupUrl -OutFile $exe
        Invoke-WebRequest -UseBasicParsing -Uri "$RustupUrl.sha256" -OutFile "$exe.sha256"
    } catch { Add-Result 'rustup' 'BŁĄD' "pobieranie: $($_.Exception.Message)"; return }
    finally { $ProgressPreference = $previous }
    $expected = [regex]::Match((Get-Content -LiteralPath "$exe.sha256" -Raw), '[0-9a-fA-F]{64}').Value.ToLowerInvariant()
    $actual = (Get-FileHash -LiteralPath $exe -Algorithm SHA256).Hash.ToLowerInvariant()
    if (-not $expected -or $expected -ne $actual) { Add-Result 'rustup' 'BŁĄD' "suma SHA-256 się nie zgadza ($actual)"; return }
    & $exe -y --default-toolchain none --profile minimal
    $code = $LASTEXITCODE
    Update-SessionPath
    if ($code -eq 0) { Add-Result 'rustup' 'OK' $RustDirs.CARGO_HOME } else { Add-Result 'rustup' "BŁĄD (kod $code)" 'rustup-init' }
}

function Install-Rust {
    $existing = Get-Command 'rustup' -ErrorAction SilentlyContinue
    if ($existing -and -not ($existing.Source -like "$ToolsDir\*")) {
        Add-Result 'Rust (rustup)' 'OK' "już zainstalowany: $($existing.Source) — zostaje na miejscu"
    } else {
        foreach ($name in @($RustDirs.Keys)) { if (-not (Set-UserVariable $name $RustDirs[$name])) { return } }
        if (-not $existing) { Install-Rustup }
    }
    $rustupReady = [bool](Get-Command 'rustup' -ErrorAction SilentlyContinue)
    if (-not $rustupReady -and -not $Ci) { return }
    $chains = if ($rustupReady) { Get-Output 'rustup' @('toolchain', 'list') } else { '' }
    $components = if ($chains -match "(?m)^$([regex]::Escape($Toolchain))") { Get-Output 'rustup' @('component', 'list', '--installed', '--toolchain', $Toolchain) } else { '' }
    if (($components -match 'rustfmt') -and ($components -match 'clippy')) { Add-Result 'Rust 1.94 z rustfmt i clippy' 'OK' $Toolchain; return }
    if ($Ci) { Add-Result 'Rust 1.94 z rustfmt i clippy' 'PLAN' "rustup toolchain install $Toolchain"; return }
    if (-not (Confirm-Step "Zainstalować Rust $Toolchain?")) { Add-Result 'Rust 1.94 z rustfmt i clippy' 'pominięty'; return }
    & rustup toolchain install $Toolchain --profile minimal --component rustfmt --component clippy
    if ($LASTEXITCODE -eq 0) { Add-Result 'Rust 1.94 z rustfmt i clippy' 'OK' $Toolchain } else { Add-Result 'Rust 1.94 z rustfmt i clippy' "BŁĄD (kod $LASTEXITCODE)" }
}

# --- Repozytorium, npm/pnpm, budowa, skrót ------------------------------------------------------------------
function Get-Repository {
    if (Test-Path -LiteralPath (Join-Path $RepoDir '.git')) {
        if ($Ci) { Add-Result 'Repozytorium Alfy' 'PLAN' "git pull --ff-only w $RepoDir"; return }
        & git -C $RepoDir pull --ff-only
        if ($LASTEXITCODE -eq 0) { Add-Result 'Repozytorium Alfy' 'OK' "zaktualizowane: $RepoDir" } else { Add-Result 'Repozytorium Alfy' 'UWAGA' 'git pull się nie udał — zostaje obecna wersja' }
        return
    }
    if ((Test-Path -LiteralPath $RepoDir) -and @(Get-ChildItem -LiteralPath $RepoDir -Force).Count -gt 0) {
        Add-Result 'Repozytorium Alfy' 'BŁĄD' "$RepoDir nie jest pusty, a nie jest repozytorium — przenieś jego zawartość"; return
    }
    if (-not (New-OwnedDir $RepoDir 'Katalog repozytorium')) { return }
    if ($Ci) { Add-Result 'Repozytorium Alfy' 'PLAN' "git clone $RepoUrl $RepoDir"; return }
    if (-not (Get-Command 'git' -ErrorAction SilentlyContinue)) { Add-Result 'Repozytorium Alfy' 'BŁĄD' 'brak Gita'; return }
    & git clone $RepoUrl $RepoDir
    if ($LASTEXITCODE -eq 0) { Add-Result 'Repozytorium Alfy' 'OK' $RepoDir } else { Add-Result 'Repozytorium Alfy' "BŁĄD (kod $LASTEXITCODE)" 'git clone' }
}

function Set-NodeTools {
    $cache = Join-Path $ToolsDir 'npm-cache'; $store = Join-Path $ToolsDir 'pnpm-store'
    if ($Ci) { Add-Result 'npm i pnpm' 'PLAN' "cache npm $cache, magazyn pnpm $store, pnpm z package.json"; return }
    if (-not (Get-Command 'npm' -ErrorAction SilentlyContinue)) { Add-Result 'npm i pnpm' 'BŁĄD' 'brak Node.js'; return }
    if (-not (Confirm-Step "Ustawić cache npm i magazyn pnpm na $ToolsDir i zainstalować pnpm?")) { Add-Result 'npm i pnpm' 'pominięty'; return }
    & npm config set cache $cache
    $wanted = '10'
    $manifest = Join-Path $RepoDir 'package.json'
    if ((Test-Path -LiteralPath $manifest) -and ((Get-Content -LiteralPath $manifest -Raw) -match '"packageManager"\s*:\s*"pnpm@([0-9.]+)"')) { $wanted = $Matches[1] }
    Push-Location $env:TEMP; $pnpm = Get-Output 'pnpm' @('--version'); Pop-Location
    if ($pnpm -notmatch '(?m)^1\d\.') { & npm install --global "pnpm@$wanted" --no-audit --no-fund; Update-SessionPath }
    & pnpm config set store-dir $store
    if ($LASTEXITCODE -eq 0) { Add-Result 'npm i pnpm' 'OK' "pnpm $wanted, magazyn $store" } else { Add-Result 'npm i pnpm' "BŁĄD (kod $LASTEXITCODE)" 'pnpm config' }
}

function Invoke-SetupDev([string[]]$Switches, [string]$Title) {
    $setup = Join-Path $RepoDir 'scripts\setup-dev.ps1'
    if ($Ci) { Add-Result $Title 'PLAN' "setup-dev.ps1 $($Switches -join ' ')"; return }
    if (-not (Test-Path -LiteralPath $setup)) { Add-Result $Title 'BŁĄD' "brak $setup"; return }
    $engine = (Get-Process -Id $PID).Path
    & $engine -NoProfile -ExecutionPolicy Bypass -File $setup @Switches
    if ($LASTEXITCODE -eq 0) { Add-Result $Title 'OK' } else { Add-Result $Title "BŁĄD (kod $LASTEXITCODE)" 'szczegóły w podsumowaniu setup-dev.ps1 powyżej' }
}

function New-DesktopShortcut([string]$Name, [string]$Script, [string]$ScriptArgs = '', [switch]$KeepOpen) {
    # Skróty na Pulpicie: uruchomienie Alfy, aktualizacja kodu, praca nad kodem z Claude Code (na żywo).
    $path = Join-Path ([Environment]::GetFolderPath('Desktop')) "$Name.lnk"
    # Typograficzne cudzysłowy tylko w napisach w apostrofach: PowerShell traktuje „ ” jak `"`.
    $title = 'Skrót „' + $Name + '”'
    if ($Ci) { Add-Result $title 'PLAN' $path; return }
    if (Test-Path -LiteralPath $path) { Add-Result $title 'OK' 'już jest'; return }
    if (-not (Confirm-Step ('Założyć na Pulpicie skrót „' + $Name + '”?'))) { Add-Result $title 'pominięty'; return }
    $noExit = if ($KeepOpen) { '-NoExit ' } else { '' }
    $link = (New-Object -ComObject WScript.Shell).CreateShortcut($path)
    $link.TargetPath = Join-Path $env:SystemRoot 'System32\WindowsPowerShell\v1.0\powershell.exe'
    $link.Arguments = ("-NoProfile -ExecutionPolicy Bypass $noExit-File `"$RepoDir\scripts\$Script`" $ScriptArgs").Trim()
    $link.WorkingDirectory = $RepoDir
    $link.Save()
    Add-Result $title 'OK' $path
}

function New-DesktopShortcuts {
    New-DesktopShortcut 'Alfa (tryb deweloperski)' 'setup-dev.ps1' '-Run'
    New-DesktopShortcut 'Alfa — aktualizuj kod' 'update-dev.ps1'
    New-DesktopShortcut 'Alfa — praca nad kodem (Claude Code)' 'code-session.ps1' -KeepOpen
}

# --- Start ------------------------------------------------------------------------------------------------
if ($env:OS -ne 'Windows_NT') { Write-Host 'Ten skrypt jest przeznaczony dla Windows 11.' -ForegroundColor Red; exit 1 }
$principal = New-Object Security.Principal.WindowsPrincipal ([Security.Principal.WindowsIdentity]::GetCurrent())
if ($principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator) -and -not $Ci) {
    Write-Host 'Uruchom skrypt w zwykłym oknie Terminala (bez Uruchom jako administrator); instalatory same poproszą o zgodę.' -ForegroundColor Red; exit 1
}
$mode = if ($Ci) { 'tylko plan (-Ci)' } elseif ($Yes) { 'bez pytań (-Yes)' } else { 'każdy krok po Twojej zgodzie' }
Write-Host "Alfa: instalacja na dysku ${Letter}: — tryb: $mode." -ForegroundColor Cyan
$exitCode = 0
try {
    Write-Host "`nWarunki" -ForegroundColor Cyan
    if (-not (Test-Preconditions)) {
        $exitCode = 1
    } else {
        Write-Host "`nKatalogi i programy" -ForegroundColor Cyan
        if (New-OwnedDir $ToolsDir 'Katalog narzędzi') {
            foreach ($pkg in $Packages) { Install-Package $pkg }
            Install-Rust
            Get-Repository
            Set-NodeTools
            if ($script:Reboot) {
                Write-Host "`nWindows wymaga ponownego uruchomienia. Po restarcie uruchom to samo polecenie jeszcze raz." -ForegroundColor Yellow
            } elseif ($script:Failed -eq 0) {
                Write-Host "`nSprawdzenie i budowa (setup-dev.ps1)" -ForegroundColor Cyan
                $switches = @(); if (-not $NoBuild) { $switches += '-Build' }
                Invoke-SetupDev $switches 'setup-dev.ps1 — sprawdzenie i budowa'
                New-DesktopShortcuts
            }
        }
        if ($script:Failed -gt 0) { $exitCode = 1 }
    }
    Write-Host "`nPodsumowanie" -ForegroundColor Cyan
    foreach ($r in $script:Results) { Write-Host ('  {0,-12} {1}' -f "[$($r.State)]", $r.Step) }
    Write-Host "`nNa ${Letter}: — kod i kompilacja: $RepoDir; narzędzia: $ToolsDir."
    Write-Host "Na $($env:SystemDrive) zostają: Windows SDK, Instalator Visual Studio, WebView2 i dane Alfy (%LOCALAPPDATA%\Alfa)."
    Write-Host "Uruchomienie Alfy: skrót na Pulpicie albo: powershell -NoProfile -ExecutionPolicy Bypass -File $RepoDir\scripts\setup-dev.ps1 -Run"
    Write-Host 'Praca nad kodem na żywo: skrót „Alfa — praca nad kodem (Claude Code)” przy działającej Alfie; aktualizacja: „Alfa — aktualizuj kod”.'
    if ($Run -and $exitCode -eq 0 -and -not $Ci) { Invoke-SetupDev @('-Run') 'Alfa (tryb deweloperski)' }
} catch {
    $exitCode = 2
    Write-Host "Nieoczekiwany błąd skryptu (linia $($_.InvocationInfo.ScriptLineNumber)): $($_.Exception.Message)" -ForegroundColor Red
}
if ($Ci) { Write-Host "ALFA_INSTALL_RESULT=$(@('ok', 'missing', 'error')[$exitCode])" }
exit $exitCode
