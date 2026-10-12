#!/usr/bin/env bash
# Kopia zbudowanego programu dla właściciela: `…/Alfa-uruchomiona/` obok repozytorium. Właściciel
# uruchamia Alfę stąd, więc budowanie i testy (które podmieniają plik w `target/shell`) nie
# wymagają zamykania jego okna. Wywołanie zamyka uruchomioną kopię (tylko ją — po ścieżce pliku),
# kopiuje nową wersję i uruchamia ją na profilu właściciela.
#
#   scripts/deploy-owner.sh [--no-start]
set -uo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SRC="$ROOT/target/shell/debug"
DST="$(cd "$ROOT/.." && pwd)/Alfa-uruchomiona"
[ -f "$SRC/alfa-desktop.exe" ] || { echo "Brak zbudowanej powłoki w $SRC"; exit 2; }
WDST="$(cygpath -w "$DST")"
# Zamknięcie tylko procesu uruchomionego z katalogu docelowego albo z katalogu budowy (stary sposób).
powershell -NoProfile -Command "Get-Process alfa-desktop -ErrorAction SilentlyContinue | Where-Object { \$_.Path -and (\$_.Path.StartsWith('$WDST') -or \$_.Path.StartsWith('$(cygpath -w "$SRC")')) -and -not \$_.Path.Contains('trial-profile') } | ForEach-Object { taskkill.exe /PID \$_.Id /T /F | Out-Null }"
sleep 2
mkdir -p "$DST"
cp -f "$SRC/alfa-desktop.exe" "$DST/"
[ -f "$SRC/alfa_desktop_lib.dll" ] && cp -f "$SRC/alfa_desktop_lib.dll" "$DST/"
rm -rf "$DST/sidecars.nowe" && cp -r "$SRC/sidecars" "$DST/sidecars.nowe" && rm -rf "$DST/sidecars" && mv "$DST/sidecars.nowe" "$DST/sidecars"
echo "skopiowano do $DST"
if [ "${1:-}" != "--no-start" ]; then
  (cd "$DST" && (./alfa-desktop.exe >/dev/null 2>&1 &))
  echo "uruchomiono na profilu właściciela"
fi
