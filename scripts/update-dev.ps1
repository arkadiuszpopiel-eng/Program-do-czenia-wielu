<#
.SYNOPSIS
    Alfa: aktualizacja kodu (git pull) i przebudowa — skrót „Alfa — aktualizuj kod" na Pulpicie.
.DESCRIPTION
    Pobiera najnowszy kod z GitHuba (tylko szybkie przewinięcie, bez nadpisywania Twoich zmian),
    potem uruchamia scripts\setup-dev.ps1 -Build. Zmiany Cargo.lock zrobione przez kompilację
    odkłada na bok (git stash). Gdy w katalogu są inne lokalne zmiany (np. z pracy z Claude Code),
    git pull może odmówić — skrypt wtedy mówi, co zrobić, i niczego nie kasuje.
    Plik w UTF-8 z BOM (Windows PowerShell 5.1 i polskie znaki).
.EXAMPLE
    powershell -NoProfile -ExecutionPolicy Bypass -File D:\alfa\scripts\update-dev.ps1
#>
[CmdletBinding()]
param([switch]$NoBuild, [switch]$NoPause)

Set-StrictMode -Version 1.0
$ErrorActionPreference = 'Continue'
$Root = Split-Path -Parent $PSScriptRoot
$exitCode = 0
Push-Location $Root
try {
    Write-Host "Alfa: aktualizacja kodu w $Root" -ForegroundColor Cyan
    # Pliki Cargo.lock zmienia sama kompilacja (lista wersji bibliotek), nie człowiek. Taka zmiana
    # blokowała `git pull` — odkładamy ją na bok (`git stash`, do odzyskania), nie kasujemy.
    $locks = @(& git status --porcelain -- 'Cargo.lock' 'apps/desktop/src-tauri/Cargo.lock' 2>$null |
        Where-Object { $_ -match '^ M ' } | ForEach-Object { $_.Substring(3) })
    if ($locks.Count -gt 0) {
        Write-Host "Odkładam na bok zmiany z kompilacji: $($locks -join ', ') (git stash)." -ForegroundColor Yellow
        & git stash push -m 'update-dev: Cargo.lock z kompilacji' -- @locks
    }
    $changes = @(& git status --porcelain 2>$null)
    if ($changes.Count -gt 0) {
        Write-Host "Lokalne zmiany w kodzie: $($changes.Count) plików (np. z pracy z Claude Code)." -ForegroundColor Yellow
        Write-Host 'Pobieram tylko, jeśli nie kolidują z nimi; niczego nie nadpisuję.'
    }
    & git pull --ff-only
    if ($LASTEXITCODE -ne 0) {
        $exitCode = 1
        Write-Host 'Nie udało się pobrać aktualizacji bez ryzyka dla Twoich zmian.' -ForegroundColor Red
        Write-Host 'Co zrobić: zapisz swoje zmiany (git commit) albo poproś Claude Code o scalenie z najnowszą wersją.'
    } elseif (-not $NoBuild) {
        $engine = (Get-Process -Id $PID).Path
        & $engine -NoProfile -ExecutionPolicy Bypass -File (Join-Path $Root 'scripts\setup-dev.ps1') -Build
        $exitCode = $LASTEXITCODE
    }
    if ($exitCode -eq 0) {
        Write-Host ''
        Write-Host 'Gotowe. Uruchom Alfę skrótem „Alfa (tryb deweloperski)”.' -ForegroundColor Green
    }
} finally {
    Pop-Location
}
if (-not $NoPause) { [void](Read-Host 'Naciśnij Enter, aby zamknąć') }
exit $exitCode
