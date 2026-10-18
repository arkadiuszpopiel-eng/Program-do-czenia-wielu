#!/usr/bin/env bash
# Test sprawdzeń `scripts/backup.sh` bez bats — WYŁĄCZNIE z `--dry-run`, prawdziwej kopii nie robi.
# Skrypt jest kopiowany do piaskownicy w katalogu tymczasowym (sztuczny projekt ~200 KB), pamięć asystenta
# podmieniona na pusty katalog. Wymaga Git Bash na Windows (cygpath, cmd, robocopy) — gdzie indziej test
# jest pomijany. Przypadki z drugim dyskiem wymagają, żeby katalog tymczasowy i repozytorium (albo inny
# podłączony dysk) leżały na różnych dyskach; inaczej są pomijane.
#
#   bash scripts/tests/backup.test.sh
set -uo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=lib.sh
source "$HERE/lib.sh"
echo "backup.sh (--dry-run)"
if ! command -v cygpath >/dev/null || ! command -v robocopy >/dev/null; then
  echo "  pominięto: brak cygpath/robocopy (to nie jest Git Bash na Windows)"
  exit 0
fi
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT
SB="$TMP/projekt"
mkdir -p "$SB/alfa/scripts" "$SB/alfa/target" "$SB/memory" "$TMP/asystent" "$TMP/atrapy" "$TMP/bez-zapisu"
cp "$HERE/../backup.sh" "$SB/alfa/scripts/"
head -c 204800 /dev/zero >"$SB/alfa/dane.bin"
head -c 1048576 /dev/zero >"$SB/alfa/target/pominiety.bin" # `target` nie wchodzi do szacunku
echo "stan" >"$SB/memory/status.md"
# Bezpiecznik na wypadek błędu w obsłudze --dry-run: atrapy `robocopy` i `git` niczego nie kopiują.
for tool in robocopy git; do
  printf '%s\n' '#!/usr/bin/env bash' "echo \"atrapa $tool: test nie robi kopii\" >&2; exit 16" >"$TMP/bez-zapisu/$tool"
  chmod +x "$TMP/bez-zapisu/$tool"
done
SAFE_PATH="$TMP/bez-zapisu:$PATH"

backup() { run env PATH="$SAFE_PATH" ALFA_ASSISTANT_MEMORY="$TMP/asystent" bash "$SB/alfa/scripts/backup.sh" "$@"; }
drive_of() { local w; w="$(cygpath -aw "$1")"; echo "${w:0:1}" | tr '[:lower:]' '[:upper:]'; }
SB_DRIVE="$(drive_of "$SB")"

backup --dry-run
check "brak celu i ALFA_BACKUP_DIR: kod 2" rc_is 2
check "brak celu: komunikat" out_has "nie podano katalogu docelowego"
backup --dry-run /c/a /c/b
check "dwa katalogi docelowe: kod 2" rc_is 2
backup --dryrun /c/a
check "nieznana opcja: kod 2" rc_is 2
backup --help
check "--help: kod 0 i opis --dry-run" eval 'rc_is 0 && out_has "--dry-run"'
backup --dry-run "$(printf '\134\134serwer\134kopie')"
check "ścieżka sieciowa \\\\serwer: odmowa (kod 1)" eval 'rc_is 1 && out_has "ścieżki sieciowe"'
backup --dry-run //serwer/kopie
check "ścieżka sieciowa //serwer: odmowa (kod 1)" eval 'rc_is 1 && out_has "ścieżki sieciowe"'

MISSING=""
for l in z y x w v u t s r q p o n m l k j i h; do
  [ -d "/$l/" ] || { MISSING="$l"; break; }
done
if [ -n "$MISSING" ]; then
  backup --dry-run "/$MISSING/Alfa"
  check "dysk ${MISSING^^}: nie istnieje — odmowa (kod 1)" eval 'rc_is 1 && out_has "dysku ${MISSING^^}: nie ma"'
fi

backup --dry-run "$SB/../kopie"
check "cel na dysku projektu: odmowa (kod 1)" eval 'rc_is 1 && out_has "tym samym dysku co projekt"'
check "cel na dysku projektu: odmowa przed szacowaniem rozmiaru" eval '! out_has "szacowany rozmiar"'
check "cel na dysku projektu: katalogu nie utworzono" test ! -e "$TMP/kopie"

# Drugi dysk: najpierw dysk repozytorium, potem pierwszy inny podłączony.
OTHER=""
REPO_DRIVE="$(drive_of "$HERE")"
if [ "$REPO_DRIVE" != "$SB_DRIVE" ]; then OTHER="$REPO_DRIVE"; else
  for l in c d e f g h i j k l m n o p q r s t u v w x y z; do
    [ "${l^^}" != "$SB_DRIVE" ] && [ -d "/$l/" ] && { OTHER="${l^^}"; break; }
  done
fi
if [ -z "$OTHER" ]; then
  echo "  pominięto przypadki z drugim dyskiem: jest tylko dysk $SB_DRIVE:"
else
  TARGET="$(cygpath -u "$OTHER:\\")alfa-test-kopia-nie-istnieje-$$"
  backup --dry-run "$TARGET"
  check "cel na dysku $OTHER: — sprawdzenia zaliczone (kod 0)" eval 'rc_is 0 && out_has "sprawdzenia: ok"'
  check "wypisuje etykietę i wolne miejsce" eval 'out_has "etykieta" && out_has "wolne"'
  check "wypisuje szacowany rozmiar i wymagane miejsce" eval 'out_has "szacowany rozmiar" && out_has "(2 ×)"'
  check "szacunek bez katalogu target (≈ 200 KB, nie 1,2 MB)" eval '[[ "$OUT" =~ pliki\ 2[0-9][0-9]\ KB ]]'
  check "zapowiada utworzenie katalogu" out_has "zostanie utworzony"
  check "--dry-run: katalogu docelowego nie utworzono" test ! -e "$TARGET"
  run env PATH="$SAFE_PATH" ALFA_BACKUP_DIR="$TARGET" ALFA_ASSISTANT_MEMORY="$TMP/asystent" \
    bash "$SB/alfa/scripts/backup.sh" --dry-run
  check "cel z ALFA_BACKUP_DIR: kod 0" eval 'rc_is 0 && out_has "sprawdzenia: ok"'
  # Atrapa `df`: na dysku zostało 10 KB — mniej niż 2 × szacunek.
  printf '%s\n' '#!/usr/bin/env bash' \
    "printf 'Filesystem 1024-blocks Used Available Capacity Mounted on\nX: 100 90 10 90%% /x\n'" \
    >"$TMP/atrapy/df"
  chmod +x "$TMP/atrapy/df"
  run env PATH="$TMP/atrapy:$SAFE_PATH" ALFA_ASSISTANT_MEMORY="$TMP/asystent" \
    bash "$SB/alfa/scripts/backup.sh" --dry-run "$TARGET"
  check "za mało miejsca: odmowa (kod 1)" eval 'rc_is 1 && out_has "za mało miejsca"'
  check "za mało miejsca: katalogu nie utworzono" test ! -e "$TARGET"
fi

finish "backup.sh (--dry-run)"
