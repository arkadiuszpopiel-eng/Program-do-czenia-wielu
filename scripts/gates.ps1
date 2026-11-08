<#
.SYNOPSIS
    Alfa: bramki jakości na Windows jednym poleceniem (odpowiednik jobów CI, uruchamiany lokalnie).
.DESCRIPTION
    Domyślnie zestaw szybki (ok. 2–3 min): format Rust (partiami — `cargo fmt --all` przekracza na Windows
    limit długości linii poleceń przy ponad 240 crate'ach), graf zależności modułów, lint i typy interfejsu,
    testy jednostkowe interfejsu, odświeżenie docs/CATALOG.md.
    -Test a,b    dodatkowo `cargo test` wskazanych crate'ów.
    -Full        dodatkowo clippy i testy całego workspace'u oraz cargo-deny (długo; ponad 25 GB na dysku).
    Korzysta z narzędzi z dysku przenośnego (scripts\portable-env.ps1). Niczego nie instaluje.
    Plik w UTF-8 z BOM (Windows PowerShell 5.1 i polskie znaki).
.EXAMPLE
    powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\gates.ps1
.EXAMPLE
    powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\gates.ps1 -Test router-impl,safety-broker-impl
#>
[CmdletBinding()]
param(
    [string[]]$Test = @(),
    [switch]$Full
)

Set-StrictMode -Version 1.0
# Z `-File` lista `-Test a,b` przychodzi jako jeden napis — dzielimy ją sami.
$Test = @($Test | ForEach-Object { $_ -split ',' } | ForEach-Object { $_.Trim() } | Where-Object { $_ })
$Root = Split-Path -Parent $PSScriptRoot
. (Join-Path $PSScriptRoot 'portable-env.ps1') -Quiet
Set-Location $Root
$ShellManifest = Join-Path $Root 'apps\desktop\src-tauri\Cargo.toml'
$Results = New-Object System.Collections.Generic.List[object]

function Invoke-Gate([string]$Name, [scriptblock]$Body) {
    Write-Host "`n== $Name" -ForegroundColor Cyan
    $watch = [System.Diagnostics.Stopwatch]::StartNew()
    $status = 'OK'
    try {
        $global:LASTEXITCODE = 0
        & $Body
        if ($LASTEXITCODE -ne 0) { $status = "BŁĄD (kod $LASTEXITCODE)" }
    } catch {
        $status = "BŁĄD ($($_.Exception.Message))"
    }
    $Results.Add([pscustomobject]@{ Bramka = $Name; Wynik = $status; Czas = ('{0:n0} s' -f $watch.Elapsed.TotalSeconds) })
}

function Skip-Gate([string]$Name, [string]$Why) {
    $Results.Add([pscustomobject]@{ Bramka = $Name; Wynik = "pominięta — $Why"; Czas = '' })
}

Invoke-Gate 'Format Rust (workspace, partiami)' {
    $crates = @(Get-ChildItem -LiteralPath (Join-Path $Root 'crates') -Directory | Where-Object { Test-Path -LiteralPath (Join-Path $_.FullName 'Cargo.toml') } | ForEach-Object { $_.Name })
    $failed = 0
    for ($i = 0; $i -lt $crates.Count; $i += 12) {
        $batch = $crates[$i..([Math]::Min($i + 11, $crates.Count - 1))]
        $packages = @(); foreach ($c in $batch) { $packages += @('-p', $c) }
        & cargo fmt @packages -- --check
        if ($LASTEXITCODE -ne 0) { $failed++ }
    }
    $global:LASTEXITCODE = $failed
}
Invoke-Gate 'Format Rust (powłoka Tauri)' { & cargo fmt --manifest-path $ShellManifest -- --check }

$bash = Get-Command 'bash' -ErrorAction SilentlyContinue
$jq = Get-Command 'jq' -ErrorAction SilentlyContinue
if ($bash -and $jq) { Invoke-Gate 'Graf zależności modułów (check-deps.sh)' { & bash scripts/check-deps.sh } }
else { Skip-Gate 'Graf zależności modułów (check-deps.sh)' 'wymaga bash i jq w PATH' }

Invoke-Gate 'Interfejs: lint (eslint, Svelte 5, CSS)' { & pnpm lint }
Invoke-Gate 'Interfejs: typy (svelte-check)' { & pnpm check }
Invoke-Gate 'Interfejs: testy jednostkowe (vitest)' { & pnpm --filter '@alfa/desktop-ui' test }
Invoke-Gate 'Numer wersji spójny we wszystkich plikach' { & node scripts/set-version.mjs --check }
Invoke-Gate 'Katalog modułów (docs/CATALOG.md)' { & node scripts/catalog.mjs }
Invoke-Gate 'Typy TS łącza zgodne ze schematem z Rusta (generated/ipc.ts)' { & node scripts/gen-ipc-types.mjs --check }
# Tylko porównanie: nieaktualna kopia w repozytorium ma zatrzymać bramkę, a nie po cichu się odświeżyć.
Invoke-Gate 'Plan w HTML: aktualność i odnośniki (docs/html)' { & node scripts/docs-html.mjs --verify }

if ($Test.Count -gt 0) {
    $packages = @(); foreach ($c in $Test) { $packages += @('-p', $c) }
    Invoke-Gate "Testy Rust: $($Test -join ', ')" { & cargo test --locked @packages }
}

if ($Full) {
    Invoke-Gate 'Clippy (workspace)' { & cargo clippy --workspace --all-targets -- -D warnings }
    Invoke-Gate 'Testy Rust (workspace)' { & cargo test --workspace }
    Invoke-Gate 'Clippy (powłoka Tauri)' { & cargo clippy --manifest-path $ShellManifest --all-targets -- -D warnings }
    if (Get-Command 'cargo-deny' -ErrorAction SilentlyContinue) { Invoke-Gate 'cargo-deny' { & cargo deny check } }
    else { Skip-Gate 'cargo-deny' 'brak narzędzia (cargo install cargo-deny --locked)' }
}

Write-Host "`nPodsumowanie" -ForegroundColor Cyan
$Results | Format-Table -AutoSize | Out-String -Width 200 | Write-Host
$bad = @($Results | Where-Object { $_.Wynik -like 'BŁĄD*' }).Count
if ($bad -gt 0) { Write-Host "Bramki z błędem: $bad" -ForegroundColor Red; exit 1 }
Write-Host 'Wszystkie uruchomione bramki zielone.' -ForegroundColor Green
