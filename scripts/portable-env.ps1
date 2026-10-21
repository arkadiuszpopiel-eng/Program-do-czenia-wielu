<#
.SYNOPSIS
    Alfa: środowisko z dysku przenośnego — narzędzia z katalogu alfa-narzedzia obok repozytorium.
.DESCRIPTION
    Ustawia w bieżącym oknie PowerShell (niczego nie zapisuje w systemie) ścieżki do narzędzi, które leżą
    obok repozytorium w `..\alfa-narzedzia`: Rust (`rustup`, `cargo`), Node.js z pnpm, Strawberry Perl,
    cache npm i magazyn pnpm. Litera dysku nie ma znaczenia — ścieżki są liczone od położenia skryptu.
    Narzędzie, którego na dysku nie ma, zostaje systemowe.
    Na każdym komputerze osobno muszą być: Visual Studio Build Tools z C++ i Windows SDK, WebView2
    i sterownik karty graficznej (sprawdza je `setup-dev.ps1`).
    Plik w UTF-8 z BOM (Windows PowerShell 5.1 i polskie znaki).
.EXAMPLE
    . .\scripts\portable-env.ps1
.EXAMPLE
    . .\scripts\portable-env.ps1 -Quiet
#>
[CmdletBinding()]
param([switch]$Quiet)

$alfaRoot = Split-Path -Parent $PSScriptRoot
$alfaTools = Join-Path (Split-Path -Parent $alfaRoot) 'alfa-narzedzia'
$alfaReport = @()

function Add-AlfaPath([string]$Dir) {
    if ((Test-Path -LiteralPath $Dir) -and (($env:Path -split ';') -notcontains $Dir)) { $env:Path = "$Dir;$env:Path" }
}

$rustup = Join-Path $alfaTools 'rustup'
$cargo = Join-Path $alfaTools 'cargo'
if ((Test-Path -LiteralPath (Join-Path $rustup 'toolchains')) -and (Test-Path -LiteralPath (Join-Path $cargo 'bin\cargo.exe'))) {
    $env:RUSTUP_HOME = $rustup
    $env:CARGO_HOME = $cargo
    Add-AlfaPath (Join-Path $cargo 'bin')
    $alfaReport += "Rust: $cargo"
} else { $alfaReport += 'Rust: systemowy (brak na dysku przenośnym)' }

# Strawberry Perl przed Perlem z Gita — skrypty OpenSSL (SQLCipher) wymagają Perla dla MSWin32.
$perl = Join-Path $alfaTools 'StrawberryPerl'
if (Test-Path -LiteralPath (Join-Path $perl 'perl\bin\perl.exe')) {
    Add-AlfaPath (Join-Path $perl 'c\bin')
    Add-AlfaPath (Join-Path $perl 'perl\bin')
    $alfaReport += "Perl: $perl"
} else { $alfaReport += 'Perl: systemowy (brak na dysku przenośnym)' }

$node = Join-Path $alfaTools 'nodejs'
if (Test-Path -LiteralPath (Join-Path $node 'node.exe')) {
    Add-AlfaPath $node
    $alfaReport += "Node.js: $node"
} else { $alfaReport += 'Node.js: systemowy (brak na dysku przenośnym)' }

# Drobne narzędzia z oficjalnych wydań (np. `jq` dla etapu `graf` w verify), sprawdzone sumą przy pobraniu.
$bin = Join-Path $alfaTools 'bin'
if (Test-Path -LiteralPath $bin) {
    Add-AlfaPath $bin
    $alfaReport += "Narzędzia: $bin"
}

if (Test-Path -LiteralPath $alfaTools) {
    $env:npm_config_cache = Join-Path $alfaTools 'npm-cache'
    $env:npm_config_store_dir = Join-Path $alfaTools 'pnpm-store'
    $alfaReport += "Magazyn pnpm: $env:npm_config_store_dir"
}

if (-not $Quiet) {
    Write-Host "Alfa: środowisko przenośne ($alfaRoot)" -ForegroundColor Cyan
    $alfaReport | ForEach-Object { Write-Host "  $_" }
}
