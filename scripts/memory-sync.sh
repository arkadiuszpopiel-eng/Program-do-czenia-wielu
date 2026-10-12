#!/usr/bin/env bash
# Kopia pamięci projektu do repozytorium: `docs/pamiec-projektu/` (zasada właściciela z 2026-10-12 —
# pamięć projektu i wszystko, co z nią związane, ma leżeć także w katalogu projektu, dobrze opisane).
# Źródła zostają tam, gdzie czytają je narzędzia; tu trafia ich kopia, więc wędruje z repozytorium.
#
#   scripts/memory-sync.sh            kopiuje i wypisuje, co się zmieniło
#
# Katalogi źródłowe:
#   ../memory/                                  stan projektu (status.md, handoff.json, decisions.md)
#   ~/.claude/projects/<projekt>/memory/        zasady pracy zapisane przez asystenta
#   ../.remember/*.md                           dziennik sesji (bez plików roboczych i logów)
# Sekrety nie mają prawa się tu znaleźć — skrypt odmawia, gdy znajdzie coś, co wygląda na klucz.
set -uo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
PARENT="$(cd "$ROOT/.." && pwd)"
DST="$ROOT/docs/pamiec-projektu"
ASSISTANT="${ALFA_ASSISTANT_MEMORY:-$HOME/.claude/projects/E--PROGRAMY-W-CZASIE-PRAC-Alfa-AI-Program/memory}"

copy_dir() { # <źródło> <cel> <wzorzec>
  local src="$1" dst="$2" pattern="$3"
  [ -d "$src" ] || { echo "pominięto (brak): $src"; return 0; }
  mkdir -p "$dst"
  # Kopia jest lustrem: plik usunięty u źródła znika też z kopii.
  find "$dst" -maxdepth 1 -type f ! -name README.md -delete
  find "$src" -maxdepth 1 -type f -name "$pattern" -exec cp -f {} "$dst/" \;
}

copy_dir "$PARENT/memory" "$DST/1-stan-projektu" "*"
copy_dir "$ASSISTANT" "$DST/2-zasady-pracy" "*.md"
copy_dir "$PARENT/.remember" "$DST/3-dziennik-sesji" "*.md"

if grep -rlE "sk-[A-Za-z0-9_-]{20,}|-----BEGIN [A-Z ]*PRIVATE KEY-----" "$DST" >/dev/null 2>&1; then
  echo "BŁĄD: w kopii pamięci jest coś, co wygląda na klucz — usuń to u źródła i powtórz."
  grep -rlE "sk-[A-Za-z0-9_-]{20,}|-----BEGIN [A-Z ]*PRIVATE KEY-----" "$DST"
  exit 1
fi
date +"%Y-%m-%d %H:%M" >"$DST/OSTATNIA-KOPIA.txt"
echo "pamięć projektu skopiowana do $DST"
(cd "$ROOT" && git status --short docs/pamiec-projektu | head -20)
