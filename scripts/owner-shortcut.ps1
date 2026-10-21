# Skrót „Alfa” na pulpicie i w menu Start do kopii właściciela (`Alfa-uruchomiona\alfa-desktop.exe`).
# Tam obok aplikacji leżą programy Jądra (alfa-broker, alfa-broker-ui, alfa-watchdog), więc działa okno
# zgód Brokera. Build testowy w `target/shell` celowo ich nie ma (Broker w procesie — wariant testów
# prawdziwej aplikacji); 2026-10-21 właściciel uruchomił go ręcznie, bo na nowym komputerze nie było
# skrótu, i każda zmiana poziomu autonomii była odrzucana. Wywołuje go `deploy-owner.sh` przy każdym
# wgraniu, więc skrót zawsze wskazuje właściwą kopię.
#
#   powershell -NoProfile -ExecutionPolicy Bypass -File scripts/owner-shortcut.ps1 [-Target <alfa-desktop.exe>]
param([string]$Target)
$ErrorActionPreference = 'Stop'
if (-not $Target) {
  $project = Split-Path (Split-Path $PSScriptRoot -Parent) -Parent
  $Target = Join-Path $project 'Alfa-uruchomiona\alfa-desktop.exe'
}
if (-not (Test-Path -LiteralPath $Target)) {
  Write-Output "brak $Target — skrót nie utworzony"
  exit 2
}
$broker = Join-Path (Split-Path $Target -Parent) 'alfa-broker.exe'
if (-not (Test-Path -LiteralPath $broker)) {
  Write-Output "brak $broker obok aplikacji — to nie jest kopia z Jądrem, skrót nie utworzony"
  exit 3
}
$shell = New-Object -ComObject WScript.Shell
$dirs = @([Environment]::GetFolderPath('Desktop'), [Environment]::GetFolderPath('Programs'))
foreach ($dir in $dirs) {
  $path = Join-Path $dir 'Alfa.lnk'
  $link = $shell.CreateShortcut($path)
  $link.TargetPath = $Target
  $link.WorkingDirectory = Split-Path $Target -Parent
  $link.IconLocation = "$Target,0"
  $link.Description = 'Alfa (z oknem zgód Brokera, folder Alfa-uruchomiona)'
  $link.Save()
  Write-Output "skrót: $path"
}
