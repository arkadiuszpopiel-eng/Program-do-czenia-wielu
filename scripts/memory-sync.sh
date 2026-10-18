#!/usr/bin/env bash
# Kopia pamięci projektu do repozytorium: `docs/pamiec-projektu/` (zasada właściciela z 2026-10-12 —
# pamięć projektu i wszystko, co z nią związane, ma leżeć także w katalogu projektu, dobrze opisane).
# Źródła zostają tam, gdzie czytają je narzędzia; tu trafia ich kopia, więc wędruje z repozytorium.
#
#   scripts/memory-sync.sh            kopiuje nowe i zmienione pliki; z kopii NICZEGO nie usuwa
#   scripts/memory-sync.sh --prune    lustro: usuwa z kopii pliki, których nie ma już u źródła
#   scripts/memory-sync.sh --help     ten opis
#
# Bez `--prune` plik, który jest tylko w kopii w repozytorium (zasady z drugiego komputera, pusta pamięć
# asystenta na nowej maszynie), zostaje na miejscu i trafia na listę OSTRZEŻEŃ z gotowym poleceniem
# przywrócenia go do źródła. Taki katalog źródłowy wygląda na niepełny, więc w jego kopii nie są też
# nadpisywane pliki, które różnią się od źródła (np. spis MEMORY.md) — nowe pliki dochodzą normalnie.
# 2026-10-17/18 lustro skasowało w ten sposób z repozytorium zasady pracy, gdy dysk przeszedł na komputer
# z pustą pamięcią asystenta. `--prune` tylko wtedy, gdy pliki usunięto u źródła celowo.
#
# Katalogi źródłowe:
#   ../memory/                                  stan projektu (status.md, handoff.json, decisions.md)
#   ~/.claude/projects/<projekt>/memory/        zasady pracy zapisane przez asystenta (ALFA_ASSISTANT_MEMORY)
#   ../.remember/*.md                           dziennik sesji (bez plików roboczych i logów)
# Sekrety nie mają prawa się tu znaleźć — skrypt odmawia, gdy znajdzie coś, co wygląda na klucz.
# Kody wyjścia: 0 — kopia zrobiona (także z ostrzeżeniami), 1 — znaleziono coś jak klucz, 2 — zły argument.
set -uo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
PARENT="$(cd "$ROOT/.." && pwd)"
DST="$ROOT/docs/pamiec-projektu"
ASSISTANT="${ALFA_ASSISTANT_MEMORY:-$HOME/.claude/projects/E--PROGRAMY-W-CZASIE-PRAC-Alfa-AI-Program/memory}"

usage() { # nagłówek tego pliku bez znaków komentarza
  awk 'NR > 1 && /^#/ { sub(/^# ?/, ""); print; next } NR > 1 { exit }' "${BASH_SOURCE[0]}"
}

PRUNE=0
for arg in "$@"; do
  case "$arg" in
    --prune) PRUNE=1 ;;
    -h | --help) usage; exit 0 ;;
    *) echo "nieznany argument: $arg (dozwolone: --prune, --help)" >&2; exit 2 ;;
  esac
done

# Ścieżka w pojedynczym cudzysłowie — polecenie do skopiowania działa także ze spacjami w ścieżce.
shq() { printf "'%s'" "${1//\'/\'\\\'\'}"; }

RESTORE=()                # polecenia przywrócenia plików, które są tylko w kopii
REPO_ONLY=0               # ile takich plików
RULES_ONLY_IN_REPO=0      # 1 = któraś zasada pracy jest tylko w kopii (trzeba połączyć też spis MEMORY.md)
HELD=()                   # polecenia porównania plików, których kopii nie nadpisano
declare -A NEEDS_MKDIR=() # brakujące katalogi źródłowe, dla których jest już `mkdir -p`

# Plik tylko w kopii: zapamiętuje polecenie przywrócenia (`cp -n` niczego u źródła nie nadpisze).
note_repo_only() { # <źródło> <plik w kopii>
  local src="$1" file="$2"
  if [ ! -d "$src" ] && [ -z "${NEEDS_MKDIR[$src]:-}" ]; then
    RESTORE+=("mkdir -p $(shq "$src")")
    NEEDS_MKDIR[$src]=1
  fi
  RESTORE+=("cp -n $(shq "$file") $(shq "$src/")")
  REPO_ONLY=$((REPO_ONLY + 1))
  [ "$src" != "$ASSISTANT" ] || RULES_ONLY_IN_REPO=1
}

copy_dir() { # <źródło> <cel> <wzorzec>
  local src="$1" dst="$2" pattern="$3" f name rel="${2#"$ROOT"/}" incomplete=0
  if [ ! -d "$src" ]; then
    # Brak źródła to typowy objaw nowej maszyny — kopia zostaje nietknięta także z `--prune`.
    echo "pominięto (brak źródła): $src"
    [ -d "$dst" ] || return 0
    while IFS= read -r -d '' f; do note_repo_only "$src" "$f"; done \
      < <(find "$dst" -maxdepth 1 -type f ! -name README.md -print0 | sort -z)
    return 0
  fi
  mkdir -p "$dst"
  # 1. Pliki, których u źródła nie ma: bez `--prune` zostają (ostrzeżenie), z `--prune` znikają (lustro).
  while IFS= read -r -d '' f; do
    name="${f##*/}"
    if [ -f "$src/$name" ] && [[ "$name" == $pattern ]]; then continue; fi
    if [ "$PRUNE" -eq 1 ]; then
      rm -f -- "$f" && echo "usunięto z kopii (--prune): $rel/$name"
    elif [ ! -f "$src/$name" ]; then
      note_repo_only "$src" "$f"
      incomplete=1
    fi
  done < <(find "$dst" -maxdepth 1 -type f ! -name README.md -print0 | sort -z)
  # 2. Nowe i zmienione pliki ze źródła (zmienione — tylko gdy źródło nie wygląda na niepełne).
  while IFS= read -r -d '' f; do
    name="${f##*/}"
    if [ ! -e "$dst/$name" ]; then
      cp -f -- "$f" "$dst/" && echo "nowy: $rel/$name"
    elif cmp -s -- "$f" "$dst/$name"; then
      continue
    elif [ "$incomplete" -eq 1 ]; then
      HELD+=("diff -u $(shq "$dst/$name") $(shq "$f")")
    else
      cp -f -- "$f" "$dst/" && echo "zaktualizowany: $rel/$name"
    fi
  done < <(find "$src" -maxdepth 1 -type f -name "$pattern" -print0 | sort -z)
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

if [ "$REPO_ONLY" -gt 0 ]; then
  echo
  echo "OSTRZEŻENIE: $REPO_ONLY plik(ów) jest tylko w kopii w repozytorium — u źródła ich nie ma, więc NIE"
  echo "zostały usunięte. Jeśli mają zostać (np. zasady z drugiego komputera), przywróć je do źródła:"
  printf '  %s\n' "${RESTORE[@]}"
  if [ "${#HELD[@]}" -gt 0 ]; then
    echo "Źródło wygląda na niepełne, więc nie nadpisano też ${#HELD[@]} plik(ów) różniących się od źródła"
    echo "(kopia w repozytorium → źródło):"
    printf '  %s\n' "${HELD[@]}"
  fi
  [ "$RULES_ONLY_IN_REPO" -eq 0 ] \
    || echo "Po przywróceniu zasad połącz u źródła spis MEMORY.md z wersją z kopii w repozytorium."
  echo "Potem uruchom ten skrypt ponownie. Jeśli pliki usunięto u źródła celowo:"
  echo "  bash scripts/memory-sync.sh --prune   (lustro: usuwa je z kopii i nadpisuje różniące się pliki)"
fi

# `safe.directory`: dysk przeniesiony z innej instalacji Windows ma pliki innego właściciela (SID).
if git -c safe.directory='*' -C "$ROOT" rev-parse --is-inside-work-tree >/dev/null 2>&1; then
  (cd "$ROOT" && git -c safe.directory='*' status --short docs/pamiec-projektu | head -20)
fi
