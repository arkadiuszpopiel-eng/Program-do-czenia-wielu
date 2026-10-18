#!/usr/bin/env bash
# Test `scripts/memory-sync.sh` bez bats: skrypt jest kopiowany do piaskownicy w katalogu tymczasowym
# (sztuczne repozytorium i sztuczne źródła, ścieżka ze spacją i apostrofem), więc test nie dotyka
# `docs/pamiec-projektu` ani prawdziwej pamięci asystenta. Działa w Git Bash i na Linuksie.
#
#   bash scripts/tests/memory-sync.test.sh
set -uo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=lib.sh
source "$HERE/lib.sh"
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT
SB="$TMP/piaskownica z ' apostrofem"
ASSISTANT="$SB/asystent"
COPY="$SB/projekt/alfa/docs/pamiec-projektu"
RULES="$COPY/2-zasady-pracy"

setup() { # świeża piaskownica: projekt/alfa (repozytorium), projekt/memory, projekt/.remember, asystent
  rm -rf "$SB"
  mkdir -p "$SB/projekt/alfa/scripts" "$SB/projekt/memory" "$SB/projekt/.remember" "$ASSISTANT" \
    "$COPY/1-stan-projektu" "$RULES" "$COPY/3-dziennik-sesji"
  cp "$HERE/../memory-sync.sh" "$SB/projekt/alfa/scripts/"
  echo "opis kopii" >"$COPY/README.md"
  echo "stan" >"$SB/projekt/memory/status.md"
  echo "dziennik" >"$SB/projekt/.remember/today.md"
}
sync() { run env ALFA_ASSISTANT_MEMORY="$ASSISTANT" bash "$SB/projekt/alfa/scripts/memory-sync.sh" "$@"; }
# Wykonuje polecenia przywrócenia wypisane w ostrzeżeniu (wiersze „  mkdir -p …” i „  cp -n …”).
restore_from_warning() {
  local line
  while IFS= read -r line; do
    case "$line" in "  mkdir -p "* | "  cp -n "*) eval "$line" || return 1 ;; esac
  done <<<"$OUT"
}
same() { cmp -s "$1" "$2"; }

echo "memory-sync.sh"

setup
echo "zasada A" >"$ASSISTANT/a.md"
echo "notatka" >"$ASSISTANT/szkic.txt"
sync
check "kod 0 przy zwykłej kopii" rc_is 0
check "nowy plik ze źródła trafia do kopii" same "$ASSISTANT/a.md" "$RULES/a.md"
check "plik spoza wzorca (*.md) nie trafia do kopii" test ! -e "$RULES/szkic.txt"
check "stan projektu i dziennik skopiowane" test -f "$COPY/1-stan-projektu/status.md" -a -f "$COPY/3-dziennik-sesji/today.md"
check "data kopii zapisana" test -s "$COPY/OSTATNIA-KOPIA.txt"
check "bez plików tylko w kopii — bez ostrzeżenia" eval '! out_has OSTRZEŻENIE'
echo "zasada A, wersja 2" >"$ASSISTANT/a.md"
sync
check "zmieniony plik jest nadpisany, gdy źródło jest pełne" same "$ASSISTANT/a.md" "$RULES/a.md"
check "wypisuje zaktualizowany plik" out_has "zaktualizowany: docs/pamiec-projektu/2-zasady-pracy/a.md"

setup
echo "zasada z drugiego komputera" >"$RULES/z-pc.md"
printf '%s\n' "- 12 wpisów" >"$RULES/MEMORY.md"
printf '%s\n' "- 2 wpisy" >"$ASSISTANT/MEMORY.md"
echo "nowa zasada" >"$ASSISTANT/nowa.md"
sync
check "bez --prune: kod 0" rc_is 0
check "bez --prune: plik tylko w kopii zostaje" test -f "$RULES/z-pc.md"
check "bez --prune: ostrzeżenie o pliku tylko w kopii" out_has "OSTRZEŻENIE: 1 plik(ów)"
check "ostrzeżenie podaje polecenie przywrócenia do pamięci asystenta" out_has "/asystent/'"
check "źródło niepełne: różniący się spis MEMORY.md nie jest nadpisany" grep -q "12 wpisów" "$RULES/MEMORY.md"
check "źródło niepełne: polecenie porównania wstrzymanego pliku" out_has "diff -u"
check "źródło niepełne: nowy plik i tak trafia do kopii" same "$ASSISTANT/nowa.md" "$RULES/nowa.md"
check "przypomnienie o połączeniu spisu MEMORY.md" out_has "spis MEMORY.md"
check "polecenia z ostrzeżenia wykonują się" restore_from_warning
check "polecenie z ostrzeżenia przywraca plik do źródła" same "$RULES/z-pc.md" "$ASSISTANT/z-pc.md"
check "przywrócenie nie nadpisuje pliku u źródła (cp -n)" grep -q "2 wpisy" "$ASSISTANT/MEMORY.md"
cp "$RULES/MEMORY.md" "$ASSISTANT/MEMORY.md"
sync
check "po przywróceniu: bez ostrzeżenia" eval '! out_has OSTRZEŻENIE'

setup
echo "zasada 1" >"$RULES/r1.md"
echo "zasada 2" >"$RULES/r2.md"
rm -rf "$ASSISTANT"
sync
check "brak katalogu źródłowego: kod 0" rc_is 0
check "brak katalogu źródłowego: kopia nietknięta" test -f "$RULES/r1.md" -a -f "$RULES/r2.md"
check "brak katalogu źródłowego: polecenie mkdir -p" out_has "mkdir -p '"
check "brak katalogu źródłowego: polecenia odtwarzają katalog" restore_from_warning
check "brak katalogu źródłowego: oba pliki przywrócone" same "$RULES/r1.md" "$ASSISTANT/r1.md"
check "mkdir -p wypisany raz na katalog" test "$(grep -c '^  mkdir -p' <<<"$OUT")" -eq 1
rm -rf "$ASSISTANT"
sync --prune
check "--prune przy braku źródła niczego nie usuwa" test -f "$RULES/r1.md" -a -f "$RULES/r2.md"

setup
echo "opis katalogu" >"$RULES/README.md"
echo "stara zasada" >"$RULES/stara.md"
echo "inna treść" >"$RULES/a.md"
echo "zasada A" >"$ASSISTANT/a.md"
sync --prune
check "--prune: kod 0" rc_is 0
check "--prune: plik tylko w kopii usunięty (lustro)" test ! -e "$RULES/stara.md"
check "--prune: wypisuje usunięty plik" out_has "usunięto z kopii (--prune): docs/pamiec-projektu/2-zasady-pracy/stara.md"
check "--prune: README.md w kopii zostaje" test -f "$RULES/README.md"
check "--prune: różniący się plik nadpisany" same "$ASSISTANT/a.md" "$RULES/a.md"

setup
printf 'klucz: %s\n' "sk-$(printf 'x%.0s' {1..24})" >"$ASSISTANT/sekret.md"
sync
check "coś jak klucz: kod 1" rc_is 1
check "coś jak klucz: komunikat BŁĄD" out_has "BŁĄD: w kopii pamięci jest coś, co wygląda na klucz"

sync --bez-sensu
check "nieznany argument: kod 2" rc_is 2
sync --help
check "--help: kod 0 i opis --prune" eval 'rc_is 0 && out_has "--prune"'

finish "memory-sync.sh"
