#!/usr/bin/env bash
# Buduje procesy poboczne mowy (alfa-supertonic, alfa-smart-turn) i kładzie je obok zbudowanego
# programu, w katalogu `sidecars/<nazwa>/`. Użycie:
#   scripts/stage-sidecars.sh [katalog programu]        (domyślnie target/shell/debug)
# Bibliotekę ONNX Runtime kopiuje, gdy podano ją w zmiennej ALFA_ORT_DLL; bez niej program użyje
# biblioteki pobranej z katalogu „Modele lokalne i silniki” (sidecars/onnxruntime/), ale wgranie
# u właściciela (deploy-owner.sh) wymaga kompletu z biblioteką obok obu procesów.
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
dest="${1:-$root/target/shell/debug}"
ext=""
case "$(uname -s)" in MINGW* | MSYS* | CYGWIN*) ext=".exe" ;; esac

cd "$root"
# Brak podanej biblioteki = błąd przed kopiowaniem: po przerwaniu w połowie zostawał komplet niepełny.
if [ -n "${ALFA_ORT_DLL:-}" ] && [ ! -f "$ALFA_ORT_DLL" ]; then
  echo "brak ALFA_ORT_DLL: $ALFA_ORT_DLL" >&2
  exit 1
fi
cargo build --release -j "${ALFA_JOBS:-3}" -p lib-supertonic -p lib-smart-turn

stage() {
  local dir="$dest/sidecars/$1" bin="$2$ext"
  mkdir -p "$dir"
  cp -f "$root/target/release/$bin" "$dir/$bin"
  if [ -n "${ALFA_ORT_DLL:-}" ]; then
    cp -f "$ALFA_ORT_DLL" "$dir/"
  fi
  echo "gotowe: $dir/$bin"
}

stage supertonic alfa-supertonic
stage smart-turn alfa-smart-turn
