#!/usr/bin/env bash
# Instancja testowa Alfy na profilu tymczasowym — OBOK uruchomionej Alfy właściciela
# (docs/plan/24 §2.7). Dane właściciela nie są dotykane: profil leży w `target/trial-profile`,
# a model i silnik rozmowy są do niego KOPIOWANE (nie dowiązywane).
#
#   scripts/trial/profile.sh start [--model|--voice|--fresh]   uruchamia (--model: kopia Bielika 4.5B i llama;
#                                                      --voice: kopia silników rozmowy)
#   scripts/trial/profile.sh stop              kończy tylko tę instancję (po numerze procesu)
#   scripts/trial/profile.sh port              wypisuje port CDP instancji testowej
#
# Wymaga powłoki zbudowanej z cechą `e2e` (`target/shell/debug/alfa-desktop.exe`).
set -uo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
PROFILE="${ALFA_TRIAL_PROFILE:-$ROOT/target/trial-profile}"
PORT="${ALFA_TRIAL_PORT:-9333}"
PIDFILE="$PROFILE/instancja.pid"
OWNER="${LOCALAPPDATA:-}/Alfa"
EXE="$ROOT/target/shell/debug/alfa-desktop.exe"

case "${1:-}" in
start)
  [ -f "$EXE" ] || { echo "Brak $EXE — najpierw budowa powłoki z cechą e2e."; exit 2; }
  # Czysty stan przy każdym starcie: zostają tylko skopiowane modele i silniki. Stare zadania
  # i wyzwalacze z poprzednich prób ruszałyby w tle i zajmowały kartę graficzną.
  # `--fresh`: profil bez żadnych modeli i silników (próba pobierania samym programem).
  if [ "${2:-}" = "--fresh" ]; then
    rm -rf "$PROFILE/Local/Alfa"
  fi
  if [ -d "$PROFILE/Local/Alfa" ]; then
    find "$PROFILE/Local/Alfa" -mindepth 1 -maxdepth 1 ! -name models ! -name sidecars -exec rm -rf {} +
  fi
  rm -rf "$PROFILE/Roaming"
  mkdir -p "$PROFILE/Roaming" "$PROFILE/Local/Alfa/models" "$PROFILE/Local/Alfa/sidecars"
  if [ "${2:-}" = "--model" ]; then
    for f in Bielik-4.5B-v3.0-Instruct.Q8_0.gguf Bielik-4.5B-v3.0-Instruct.Q8_0.gguf.sha256; do
      [ -f "$PROFILE/Local/Alfa/models/$f" ] || cp "$OWNER/models/$f" "$PROFILE/Local/Alfa/models/$f"
    done
    for d in llama-cuda llama-cpu; do
      [ -d "$PROFILE/Local/Alfa/sidecars/$d" ] || cp -r "$OWNER/sidecars/$d" "$PROFILE/Local/Alfa/sidecars/$d"
    done
  fi
  if [ "${2:-}" = "--voice" ]; then
    # Silniki rozmowy: rozpoznawanie (Qwen3-ASR + llama), wykrywanie mowy, koniec wypowiedzi, głos.
    for d in qwen3-asr silero turn supertonic whisper; do
      [ -d "$PROFILE/Local/Alfa/models/$d" ] || cp -r "$OWNER/models/$d" "$PROFILE/Local/Alfa/models/$d"
    done
    for d in llama-cuda llama-cpu supertonic smart-turn onnxruntime whisper whisper-cuda; do
      [ -d "$PROFILE/Local/Alfa/sidecars/$d" ] || cp -r "$OWNER/sidecars/$d" "$PROFILE/Local/Alfa/sidecars/$d"
    done
  fi
  APPDATA="$(cygpath -w "$PROFILE/Roaming")" LOCALAPPDATA="$(cygpath -w "$PROFILE/Local")" \
    ALFA_E2E_INSTANCE=1 ALFA_E2E_CDP_PORT="$PORT" "$EXE" >"$PROFILE/instancja.log" 2>&1 &
  echo $! >"$PIDFILE"
  for _ in $(seq 1 90); do
    curl -s "http://127.0.0.1:$PORT/json/version" >/dev/null 2>&1 && { echo "gotowa: port $PORT"; exit 0; }
    sleep 1
  done
  echo "Instancja testowa nie otworzyła portu $PORT w 90 s."; exit 1
  ;;
stop)
  # Tylko własny proces i jego potomkowie — nigdy `taskkill /IM`, bo zamknęłoby Alfę właściciela.
  if [ -f "$PIDFILE" ]; then
    WINPID="$(cat /proc/"$(cat "$PIDFILE")"/winpid 2>/dev/null || true)"
    [ -n "$WINPID" ] && taskkill //F //T //PID "$WINPID" >/dev/null 2>&1
    rm -f "$PIDFILE"
  fi
  echo "zatrzymana"
  ;;
port) echo "$PORT" ;;
*) echo "Użycie: $0 start [--model] | stop | port"; exit 2 ;;
esac
