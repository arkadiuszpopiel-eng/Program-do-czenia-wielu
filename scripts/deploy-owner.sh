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
# Programy Jądra mają osobny katalog budowy: obok powłoki w `target/shell` przełączyłyby testy
# prawdziwej aplikacji (`verify-app.sh`, wariant „Broker w procesie”) w tryb przenośny.
KERNEL="$ROOT/target/safety/debug"
DST="$(cd "$ROOT/.." && pwd)/Alfa-uruchomiona"
[ -f "$SRC/alfa-desktop.exe" ] || { echo "Brak zbudowanej powłoki w $SRC"; exit 2; }
(
  cd "$ROOT" && source scripts/portable-env.sh >/dev/null 2>&1
  CARGO_TARGET_DIR="$(cygpath -w "$ROOT/target/safety")" CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0 \
    cargo build --locked -j 3 -p app-safety --bins >"$ROOT/target/safety-build.log" 2>&1
) || { echo "budowa programów Jądra: błąd (target/safety-build.log)"; exit 1; }
WDST="$(cygpath -w "$DST")"
# Zamknięcie tylko procesu uruchomionego z katalogu docelowego albo z katalogu budowy (stary sposób).
# `Stop-Process` po PID: `taskkill /T` przekraczał czas przy procesach Jądra w zadaniu watchdoga
# (2026-10-20), a stara Alfa zostawała uruchomiona. Broker i watchdog kończą się z aplikacją.
powershell -NoProfile -Command "Get-Process alfa-desktop -ErrorAction SilentlyContinue | Where-Object { \$_.Path -and (\$_.Path.StartsWith('$WDST') -or \$_.Path.StartsWith('$(cygpath -w "$SRC")')) -and -not \$_.Path.Contains('trial-profile') } | ForEach-Object { Stop-Process -Id \$_.Id -Force }"
sleep 4
# Procesy Jądra z katalogu docelowego, które przeżyły aplikację, blokowałyby kopiowanie plików.
powershell -NoProfile -Command "Get-Process alfa-broker,alfa-broker-ui,alfa-watchdog -ErrorAction SilentlyContinue | Where-Object { \$_.Path -and \$_.Path.StartsWith('$WDST') } | ForEach-Object { Stop-Process -Id \$_.Id -Force }"
sleep 1
mkdir -p "$DST"
cp -f "$SRC/alfa-desktop.exe" "$DST/"
[ -f "$SRC/alfa_desktop_lib.dll" ] && cp -f "$SRC/alfa_desktop_lib.dll" "$DST/"
# Jądro bezpieczeństwa w trybie przenośnym (ADR 0003, `app_broker::mode`): Broker, okno zatwierdzeń
# i watchdog obok aplikacji. Bez nich Broker działa w procesie, bez okna — podniesienia poziomu
# autonomii są wtedy zawsze odrzucane, a kill-switch obsługuje sama aplikacja.
for bin in alfa-broker alfa-broker-ui alfa-watchdog; do
  cp -f "$KERNEL/$bin.exe" "$DST/" || { echo "brak $KERNEL/$bin.exe"; exit 1; }
done
rm -rf "$DST/sidecars.nowe" && cp -r "$SRC/sidecars" "$DST/sidecars.nowe" && rm -rf "$DST/sidecars" && mv "$DST/sidecars.nowe" "$DST/sidecars"
echo "skopiowano do $DST"
if [ "${1:-}" != "--no-start" ]; then
  (cd "$DST" && (./alfa-desktop.exe >/dev/null 2>&1 &))
  echo "uruchomiono na profilu właściciela"
fi
