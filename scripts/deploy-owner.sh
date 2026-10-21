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
# Procesy poboczne mowy zastępują kopię właściciela w całości — niepełny komplet (21.10: bez
# onnxruntime.dll i bez smart-turn) odebrałby mu głos. Sprawdzenie przed zamknięciem jego Alfy.
for part in supertonic/alfa-supertonic.exe supertonic/onnxruntime.dll smart-turn/alfa-smart-turn.exe smart-turn/onnxruntime.dll; do
  [ -f "$SRC/sidecars/$part" ] || { echo "Niepełne procesy poboczne mowy: brak $SRC/sidecars/$part (scripts/verify-app.sh)"; exit 2; }
done
(
  cd "$ROOT" && source scripts/portable-env.sh >/dev/null 2>&1
  CARGO_TARGET_DIR="$(cygpath -w "$ROOT/target/safety")" CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0 \
    cargo build --locked -j 3 -p app-safety --bins >"$ROOT/target/safety-build.log" 2>&1
) || { echo "budowa programów Jądra: błąd (target/safety-build.log)"; exit 1; }
WDST="$(cygpath -w "$DST")"
# Zamknięcie tylko procesu uruchomionego z katalogu docelowego albo z katalogu budowy (stary sposób).
# Zamyka też instancję testową `verify-app.sh` (ten sam plik w `target/shell`, inny profil) — nie
# wgrywać w trakcie testu prawdziwej aplikacji. `Stop-Process` po PID: `taskkill /T` przekraczał czas
# przy procesach Jądra w zadaniu watchdoga (2026-10-20), a stara Alfa zostawała uruchomiona. Broker
# i watchdog kończą się z aplikacją.
powershell -NoProfile -Command "Get-Process alfa-desktop -ErrorAction SilentlyContinue | Where-Object { \$_.Path -and (\$_.Path.StartsWith('$WDST') -or \$_.Path.StartsWith('$(cygpath -w "$SRC")')) } | ForEach-Object { Stop-Process -Id \$_.Id -Force }"
sleep 4
# Procesy Jądra z katalogu docelowego, które przeżyły aplikację, blokowałyby kopiowanie plików.
powershell -NoProfile -Command "Get-Process alfa-broker,alfa-broker-ui,alfa-watchdog -ErrorAction SilentlyContinue | Where-Object { \$_.Path -and \$_.Path.StartsWith('$WDST') } | ForEach-Object { Stop-Process -Id \$_.Id -Force }"
sleep 1
# Błąd kopiowania nie kończy skryptu przed uruchomieniem: właściciel nie może zostać bez Alfy
# (jego kopia jest już zamknięta). Wynik niepełny — kod 1 na końcu.
fail=0
mkdir -p "$DST"
cp -f "$SRC/alfa-desktop.exe" "$DST/" || { echo "nie skopiowano alfa-desktop.exe"; fail=1; }
if [ -f "$SRC/alfa_desktop_lib.dll" ]; then
  cp -f "$SRC/alfa_desktop_lib.dll" "$DST/" || { echo "nie skopiowano alfa_desktop_lib.dll"; fail=1; }
fi
# Jądro bezpieczeństwa w trybie przenośnym (ADR 0003, `app_broker::mode`): Broker, okno zatwierdzeń
# i watchdog obok aplikacji. Bez nich Broker działa w procesie, bez okna — podniesienia poziomu
# autonomii są wtedy zawsze odrzucane, a kill-switch obsługuje sama aplikacja.
for bin in alfa-broker alfa-broker-ui alfa-watchdog; do
  cp -f "$KERNEL/$bin.exe" "$DST/" || { echo "nie skopiowano $KERNEL/$bin.exe"; fail=1; }
done
rm -rf "$DST/sidecars.nowe" && cp -r "$SRC/sidecars" "$DST/sidecars.nowe" && rm -rf "$DST/sidecars" &&
  mv "$DST/sidecars.nowe" "$DST/sidecars" || { echo "nie podmieniono procesów pobocznych mowy"; fail=1; }
[ "$fail" -eq 0 ] && echo "skopiowano do $DST" || echo "WGRANIE NIEPEŁNE — uruchamiam to, co jest w $DST"
# Skrót „Alfa” (pulpit, menu Start) zawsze do tej kopii — nie do buildu testowego w `target/shell`.
powershell -NoProfile -ExecutionPolicy Bypass -File "$(cygpath -w "$ROOT/scripts/owner-shortcut.ps1")" \
  -Target "$WDST\\alfa-desktop.exe" || echo "skrót „Alfa”: nie utworzony"
if [ "${1:-}" != "--no-start" ]; then
  (cd "$DST" && (./alfa-desktop.exe >/dev/null 2>&1 &))
  echo "uruchomiono na profilu właściciela"
fi
exit "$fail"
