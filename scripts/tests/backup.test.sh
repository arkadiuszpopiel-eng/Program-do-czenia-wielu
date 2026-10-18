#!/usr/bin/env bash
# Test `scripts/backup.sh` bez bats. Skrypt jest kopiowany do piaskownicy w katalogu tymczasowym (sztuczny
# projekt ~200 KB z prawdziwym repozytorium git), pamięć asystenta podmieniona na pusty katalog, a `robocopy`
# na atrapę, która zapisuje tylko znacznik i swoje argumenty. Sprawdzenia przechodzą z `--dry-run`; prawdziwa
# kopia (z atrapą robocopy i prawdziwą paczką git) trafia do `target/backup-test-<pid>` repozytorium, gdy
# leży ono na innym dysku niż katalog tymczasowy, i jest usuwana na końcu. Atrapy `cmd` i `powershell.exe`
# udają ten sam wolumin i ten sam dysk fizyczny. Wymaga Git Bash na Windows (cygpath, cmd, robocopy) —
# gdzie indziej test jest pomijany; przypadki z drugim dyskiem są pomijane, gdy jest tylko jeden dysk.
#
#   bash scripts/tests/backup.test.sh
set -uo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$HERE/../.." && pwd)"
# shellcheck source=lib.sh
source "$HERE/lib.sh"
echo "backup.sh"
if ! command -v cygpath >/dev/null || ! command -v robocopy >/dev/null; then
  echo "  pominięto: brak cygpath/robocopy (to nie jest Git Bash na Windows)"
  exit 0
fi
TMP="$(mktemp -d)"
REAL_BASE=""
MADE_TARGET=0
cleanup() {
  rm -rf "$TMP"
  [ -z "$REAL_BASE" ] || rm -rf "$REAL_BASE"
  [ "$MADE_TARGET" -eq 0 ] || rmdir "$REPO_ROOT/target" 2>/dev/null
}
trap cleanup EXIT
SB="$TMP/projekt"
mkdir -p "$SB/alfa/scripts" "$SB/alfa/target" "$SB/memory" "$TMP/asystent" \
  "$TMP/atrapy" "$TMP/atrapy-df" "$TMP/atrapy-vol" "$TMP/atrapy-dysk"
cp "$HERE/../backup.sh" "$SB/alfa/scripts/"
head -c 204800 /dev/urandom >"$SB/alfa/dane.bin" # losowe — paczka git ma realny rozmiar
head -c 1048576 /dev/zero >"$SB/alfa/target/pominiety.bin" # `target` nie wchodzi do szacunku ani kopii
echo "stan" >"$SB/memory/status.md"
echo "plan" >"$SB/PLAN-KONTYNUACJI.md"
(cd "$SB/alfa" && git init -q && git config core.autocrlf false && git add dane.bin \
  && git -c user.name=test -c user.email=test@example.invalid commit -qm "dane testowe") \
  || { echo "  BŁĄD: nie udało się utworzyć repozytorium w piaskownicy"; exit 1; }

# Atrapa `robocopy`: niczego nie kopiuje — zapisuje znacznik w katalogu docelowym i argumenty w dzienniku.
export ROBOCOPY_LOG="$TMP/robocopy.log"
cat >"$TMP/atrapy/robocopy" <<'EOF'
#!/usr/bin/env bash
dst="$(cygpath -u "$2")"
mkdir -p "$dst" && printf '%s\n' "$*" >"$dst/robocopy-atrapa.txt" && printf '%s\n' "$*" >>"$ROBOCOPY_LOG"
exit "${ATRAPA_ROBOCOPY_KOD:-1}"
EOF
# Atrapa `df`: na dysku zostało 10 KB — mniej niż 2 × szacunek.
printf '%s\n' '#!/usr/bin/env bash' \
  "printf 'Filesystem 1024-blocks Used Available Capacity Mounted on\nX: 100 90 10 90%% /x\n'" >"$TMP/atrapy-df/df"
# Atrapa `cmd`: każdy dysk ma ten sam numer seryjny (jak dysk z `subst`).
printf '%s\n' '#!/usr/bin/env bash' 'echo " Volume Serial Number is 1234-ABCD"' >"$TMP/atrapy-vol/cmd"
# Atrapa `powershell.exe`: cel i projekt na tym samym dysku fizycznym (inne partycje).
printf '%s\n' '#!/usr/bin/env bash' 'printf "label=ATRAPA\ndisk=7\nproject_disk=7\n"' >"$TMP/atrapy-dysk/powershell.exe"
chmod +x "$TMP/atrapy/robocopy" "$TMP/atrapy-df/df" "$TMP/atrapy-vol/cmd" "$TMP/atrapy-dysk/powershell.exe"
SAFE_PATH="$TMP/atrapy:$PATH"

backup() { run env PATH="$SAFE_PATH" ALFA_ASSISTANT_MEMORY="$TMP/asystent" bash "$SB/alfa/scripts/backup.sh" "$@"; }
backup_with() { # <katalog atrap> <argumenty…>
  local stubs="$1"
  shift
  run env PATH="$stubs:$SAFE_PATH" ALFA_ASSISTANT_MEMORY="$TMP/asystent" bash "$SB/alfa/scripts/backup.sh" "$@"
}
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
check "--help: kod 0, opis --dry-run i kodu 3" eval 'rc_is 0 && out_has "--dry-run" && out_has "3 — kopia NIEPEŁNA"'
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
REPO_DRIVE="$(drive_of "$REPO_ROOT")"
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
  check "wypisuje etykietę, dysk fizyczny i wolne miejsce" \
    eval 'out_has "etykieta" && out_has "dysk fizyczny" && out_has "wolne"'
  check "wypisuje szacowany rozmiar i wymagane miejsce" eval 'out_has "szacowany rozmiar" && out_has "(2 ×)"'
  check "szacunek bez katalogu target (≈ 200 KB, nie 1,2 MB)" eval '[[ "$OUT" =~ pliki\ 2[0-9][0-9]\ KB ]]'
  check "szacunek z historią git (prawdziwe repozytorium)" eval '[[ "$OUT" =~ historia\ git\ [1-9][0-9]*\ KB ]]'
  check "zapowiada utworzenie katalogu" out_has "zostanie utworzony"
  check "--dry-run: katalogu docelowego nie utworzono" test ! -e "$TARGET"
  run env PATH="$SAFE_PATH" ALFA_BACKUP_DIR="$TARGET" ALFA_ASSISTANT_MEMORY="$TMP/asystent" \
    bash "$SB/alfa/scripts/backup.sh" --dry-run
  check "cel z ALFA_BACKUP_DIR: kod 0" eval 'rc_is 0 && out_has "sprawdzenia: ok"'
  backup_with "$TMP/atrapy-df" --dry-run "$TARGET"
  check "za mało miejsca: odmowa (kod 1)" eval 'rc_is 1 && out_has "za mało miejsca"'
  check "za mało miejsca: katalogu nie utworzono" test ! -e "$TARGET"
  backup_with "$TMP/atrapy-vol" --dry-run "$TARGET"
  check "ten sam numer seryjny (subst): odmowa (kod 1)" eval 'rc_is 1 && out_has "ten sam wolumin"'
  backup_with "$TMP/atrapy-dysk" --dry-run "$TARGET"
  check "inna partycja tego samego dysku fizycznego: odmowa (kod 1)" \
    eval 'rc_is 1 && out_has "tego samego dysku fizycznego (dysk 7)"'
  check "odmowa przed utworzeniem katalogu" test ! -e "$TARGET"
fi

# Prawdziwa kopia (atrapa robocopy) do target/ repozytorium — tylko gdy leży na innym dysku niż piaskownica.
if [ "$REPO_DRIVE" = "$SB_DRIVE" ]; then
  echo "  pominięto prawdziwą kopię: repozytorium i katalog tymczasowy są na tym samym dysku $SB_DRIVE:"
else
  [ -d "$REPO_ROOT/target" ] || MADE_TARGET=1
  REAL_BASE="$REPO_ROOT/target/backup-test-$$"
  backup "$REAL_BASE/ok"
  check "kopia: kod 0 i „gotowe”" eval 'rc_is 0 && out_has "gotowe: "'
  K="$(find "$REAL_BASE/ok" -maxdepth 1 -type d -name 'kopia-*' | head -1)"
  check "kopia: katalog kopia-<data>" test -n "$K"
  check "kopia: paczka git zweryfikowana" eval 'out_has "paczka git: ok" && test -s "$K/alfa.bundle"'
  check "kopia: z paczki da się odtworzyć historię" \
    eval 'git clone -q "$K/alfa.bundle" "$TMP/klon" 2>/dev/null && test "$(cd "$TMP/klon" && git log -1 --format=%s)" = "dane testowe"'
  check "kopia: ODTWORZENIE.md i ostatni-commit.txt" test -s "$K/ODTWORZENIE.md" -a -s "$K/ostatni-commit.txt"
  check "kopia: sumy SHA256SUMS.txt się zgadzają" eval '(cd "$K" && sha256sum -c --quiet SHA256SUMS.txt)'
  check "kopia: robocopy wywołane dla alfa i memory" test -f "$K/alfa/robocopy-atrapa.txt" -a -f "$K/memory/robocopy-atrapa.txt"
  check "kopia: robocopy pomija target, node_modules i .git" grep -q "/XD target node_modules .git" "$ROBOCOPY_LOG"
  check "kopia: PLAN-KONTYNUACJI.md skopiowany" test -f "$K/PLAN-KONTYNUACJI.md"
  run env PATH="$SAFE_PATH" ATRAPA_ROBOCOPY_KOD=16 ALFA_ASSISTANT_MEMORY="$TMP/asystent" \
    bash "$SB/alfa/scripts/backup.sh" "$REAL_BASE/blad"
  check "robocopy kod 16: kod 3 (kopia niepełna)" eval 'rc_is 3 && out_has "KOPIA NIEPEŁNA"'
  check "robocopy kod 16: wskazuje krok" out_has "alfa: BŁĄD (robocopy 16)"
  check "robocopy kod 16: bez „gotowe”" eval '! out_has "gotowe: "'
fi

finish "backup.sh"
