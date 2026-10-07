<#
.SYNOPSIS
    Alfa: praca nad kodem na żywo — Claude Code w katalogu repozytorium (skrót na Pulpicie).
.DESCRIPTION
    Otwiera sesję `claude` (Claude Code, na Twoim koncie — logujesz się sam; Alfa nie czyta tokenów)
    w katalogu repozytorium Alfy. Gdy obok działa Alfa w trybie deweloperskim (skrót „Alfa (tryb
    deweloperski)”), zmiany interfejsu widać od razu (Vite HMR), a zmiany w Rust przebudowują
    i restartują aplikację automatycznie (tauri dev). Bez Claude Code — instrukcja instalacji.
    Plik w UTF-8 z BOM (Windows PowerShell 5.1 i polskie znaki).
.EXAMPLE
    powershell -NoProfile -ExecutionPolicy Bypass -NoExit -File D:\alfa\scripts\code-session.ps1
#>
[CmdletBinding()]
param()

Set-StrictMode -Version 1.0
$Root = Split-Path -Parent $PSScriptRoot
Set-Location $Root
Write-Host "Alfa: praca nad kodem w $Root" -ForegroundColor Cyan
Write-Host 'Wskazówka: najpierw uruchom Alfę skrótem „Alfa (tryb deweloperski)” — zmiany zobaczysz na żywo.'
Write-Host 'Pierwsze polecenie dla Claude Code może brzmieć np.: „przeczytaj AGENTS.md i docs/STATUS.md”.'
if (Get-Command 'claude' -ErrorAction SilentlyContinue) {
    & claude
} else {
    Write-Host "`nNie znaleziono Claude Code (polecenie claude)." -ForegroundColor Yellow
    Write-Host 'Instalacja (raz): npm install -g @anthropic-ai/claude-code   — potem zaloguj się poleceniem: claude'
}
