#!/usr/bin/env bash
# Kopia bezpieczeństwa projektu na inny dysk (polecenie właściciela 2026-10-15).
# Zawartość: paczka git z pełną historią (wszystkie gałęzie), źródła bez plików kompilacji
# (`target`, `node_modules`), zbudowany program (`Alfa-uruchomiona`), pamięć projektu, notatki
# sesji, plan i audyt, sumy SHA-256 i instrukcja odtworzenia. Niczego nie usuwa — każda kopia
# trafia do nowego katalogu z datą i godziną.
#
#   scripts/backup.sh <katalog_docelowy>            kopia, np. scripts/backup.sh /g/Alfa
#   ALFA_BACKUP_DIR=/g/Alfa scripts/backup.sh       cel ze zmiennej, gdy nie podano argumentu
#   scripts/backup.sh --dry-run <katalog_docelowy>  tylko sprawdzenia — niczego nie tworzy ani nie zapisuje
#   scripts/backup.sh --help                        ten opis
#
# Celu domyślnego nie ma: 2026-10-18 litera F: na laptopie wskazywała inny, pełny dysk niż na PC.
# Przed zapisem skrypt sprawdza: dysk celu istnieje (litera dysku, bez ścieżek sieciowych \\serwer),
# NIE jest tym samym woluminem co projekt (także przez `subst` — porównuje numery seryjne) ani inną partycją
# tego samego dysku fizycznego (`Get-Partition`), wypisuje etykietę i wolne miejsce, szacuje rozmiar kopii
# (`du` kopiowanych katalogów bez target/node_modules/.git + historia git) i odmawia, gdy wolnego miejsca
# jest mniej niż 2 × szacunek.
# Kody wyjścia: 0 — kopia kompletna (z --dry-run: sprawdzenia zaliczone), 1 — odmowa po sprawdzeniu,
# 2 — zły argument, 3 — kopia NIEPEŁNA: któryś krok się nie udał (paczka git, robocopy, sumy; lista na końcu).
set -uo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
PROJECT="$(cd "$ROOT/.." && pwd)"

usage() { # nagłówek tego pliku bez znaków komentarza
  awk 'NR > 1 && /^#/ { sub(/^# ?/, ""); print; next } NR > 1 { exit }' "${BASH_SOURCE[0]}"
}

DRY_RUN=0
BASE=""
while [ $# -gt 0 ]; do
  case "$1" in
    --dry-run) DRY_RUN=1 ;;
    -h | --help) usage; exit 0 ;;
    -*) echo "nieznana opcja: $1 (scripts/backup.sh --help)" >&2; exit 2 ;;
    *)
      [ -z "$BASE" ] || { echo "za dużo argumentów: $1 — podaj jeden katalog docelowy" >&2; exit 2; }
      BASE="$1" ;;
  esac
  shift
done
BASE="${BASE:-${ALFA_BACKUP_DIR:-}}"
if [ -z "$BASE" ]; then
  echo "BŁĄD: nie podano katalogu docelowego (argument albo zmienna ALFA_BACKUP_DIR), np.:" >&2
  echo "  bash scripts/backup.sh --dry-run /g/Alfa" >&2
  exit 2
fi
for tool in cygpath robocopy df du; do
  command -v "$tool" >/dev/null || { echo "BŁĄD: brak programu $tool — skrypt działa w Git Bash na Windows" >&2; exit 2; }
done

HOME_MEM="${ALFA_ASSISTANT_MEMORY:-$(cygpath -u "${USERPROFILE:-$HOME}")/.claude/projects/E--PROGRAMY-W-CZASIE-PRAC-Alfa-AI-Program/memory}"
# Kopiowane katalogi: <źródło>|<nazwa w kopii>. Ta sama lista służy do szacunku rozmiaru i do kopii.
DIRS=(
  "$ROOT|alfa"
  "$PROJECT/Alfa-uruchomiona|Alfa-uruchomiona"
  "$PROJECT/memory|memory"
  "$PROJECT/.remember|.remember"
  "$PROJECT/PLAN|PLAN"
  "$PROJECT/audyt-2026-10-14|audyt-2026-10-14"
  "$HOME_MEM|pamiec-claude"
)
FILES=("$PROJECT/PLAN-KONTYNUACJI.md")
# Dysk przeniesiony z innej instalacji Windows ma pliki innego właściciela (SID) i git odmawia pracy
# („dubious ownership”) — wyjątek tylko dla tych poleceń, bez zmiany konfiguracji (2026-10-17: bez niego
# paczka po cichu się nie tworzyła).
GIT=(git -c safe.directory='*' -C "$ROOT")

refuse() { echo "ODMOWA: $*"; exit 1; }
human() { # KiB → czytelnie, z przecinkiem dziesiętnym
  awk -v k="$1" 'BEGIN { split("KB MB GB TB", u, " "); i = 1
    while (k >= 1024 && i < 4) { k /= 1024; i++ }
    if (i == 1) printf "%d %s", k, u[i]; else printf "%.1f %s", k, u[i] }' | tr . ,
}
# Numer seryjny woluminu (np. 1EB6-F45B); `vol` pokazuje wolumin pod spodem także dla dysku z `subst`.
vol_serial() { cmd //c "vol $1" 2>/dev/null | tr -d '\r' | grep -oE '[0-9A-F]{4}-[0-9A-F]{4}' | head -1; }
# Jedno wywołanie PowerShell (start trwa sekundy): wiersze `label=` (etykieta celu), `disk=` i `project_disk=`
# (numery dysków fizycznych celu i projektu; puste dla `subst`, dysków sieciowych i przy błędzie).
# Limit 40 s: gdy usługa WMI w Windows stoi (2026-10-20), `Get-Partition` czekał bez końca — po limicie
# numery dysków są nieznane (jak dla `subst`), a kontrola „ten sam wolumin” (`vol`) i tak obowiązuje.
drive_facts() { # <litera celu> <litera projektu>, np. F: E:
  timeout 40 powershell.exe -NoProfile -NonInteractive -Command "[Console]::OutputEncoding = [Text.Encoding]::UTF8; \
    function Disk(\$l) { try { (Get-Partition -DriveLetter \$l -ErrorAction Stop).DiskNumber } catch { '' } }; \
    try { 'label=' + [IO.DriveInfo]::new('$1\').VolumeLabel } catch { 'label=' }; \
    'disk=' + (Disk '${1:0:1}'); 'project_disk=' + (Disk '${2:0:1}')" 2>/dev/null | tr -d '\r'
}
fact() { sed -n "s/^$1=//p" <<<"$FACTS" | head -1; } # <klucz> — wartość z wyniku drive_facts

# --- Sprawdzenia przed zapisem (z --dry-run tylko one) ---
case "$BASE" in
  //* | \\\\*) refuse "ścieżki sieciowe nie są obsługiwane ($BASE) — podaj dysk z literą (np. zamapowany udział)" ;;
esac
TARGET_WIN="$(cygpath -aw "$BASE")"
DRIVE="${TARGET_WIN:0:2}"
DRIVE="${DRIVE^^}"
[[ "$DRIVE" =~ ^[A-Z]:$ ]] || refuse "cel $TARGET_WIN nie leży na dysku z literą"
DRIVE_ROOT="$(cygpath -u "$DRIVE\\")"
echo "cel: $TARGET_WIN"
[ -d "$DRIVE_ROOT" ] || refuse "dysku $DRIVE nie ma (niepodłączony albo inna litera na tym komputerze)"

PROJECT_WIN="$(cygpath -aw "$PROJECT")"
PROJECT_DRIVE="${PROJECT_WIN:0:2}"
PROJECT_DRIVE="${PROJECT_DRIVE^^}"
[ "$DRIVE" != "$PROJECT_DRIVE" ] \
  || refuse "cel jest na tym samym dysku co projekt ($PROJECT_DRIVE) — kopia nie przetrwałaby awarii dysku"
SERIAL="$(vol_serial "$DRIVE")"
PROJECT_SERIAL="$(vol_serial "$PROJECT_DRIVE")"
if [ -n "$SERIAL" ] && [ "$SERIAL" = "$PROJECT_SERIAL" ]; then
  refuse "$DRIVE to ten sam wolumin co $PROJECT_DRIVE (numer seryjny $SERIAL, np. dysk z subst)"
fi
FACTS="$(drive_facts "$DRIVE" "$PROJECT_DRIVE")"
[ -n "$FACTS" ] || echo "uwaga: brak odpowiedzi PowerShell/WMI w 40 s — numer dysku fizycznego nieznany"
DISK="$(fact disk)"
PROJECT_DISK="$(fact project_disk)"
if [[ "$DISK" =~ ^[0-9]+$ ]] && [ "$DISK" = "$PROJECT_DISK" ]; then
  refuse "$DRIVE i $PROJECT_DRIVE to partycje tego samego dysku fizycznego (dysk $DISK) —" \
    "kopia nie przetrwałaby awarii dysku"
fi

if [ -e "$BASE" ] && [ ! -d "$BASE" ]; then refuse "$TARGET_WIN istnieje i nie jest katalogiem"; fi
[ -d "$BASE" ] || echo "katalog docelowy nie istnieje — zostanie utworzony"

LABEL="$(fact label)"
read -r FREE_KB TOTAL_KB < <(df -Pk "$DRIVE_ROOT" 2>/dev/null | awk 'NR == 2 { print $4, $2 }')
[[ "${FREE_KB:-}" =~ ^[0-9]+$ ]] || refuse "nie udało się odczytać wolnego miejsca na $DRIVE"
echo "dysk $DRIVE — etykieta „${LABEL:-(brak)}”, numer seryjny ${SERIAL:-(nieznany)}," \
  "dysk fizyczny ${DISK:-(nieznany)} (projekt: ${PROJECT_DISK:-(nieznany)})," \
  "wolne $(human "$FREE_KB") z $(human "${TOTAL_KB:-0}")"

SOURCES=()
for entry in "${DIRS[@]}"; do [ -d "${entry%%|*}" ] && SOURCES+=("${entry%%|*}"); done
for f in "${FILES[@]}"; do [ -f "$f" ] && SOURCES+=("$f"); done
SRC_KB=0
if [ "${#SOURCES[@]}" -gt 0 ]; then
  SRC_KB="$(du -sk --exclude=target --exclude=node_modules --exclude=.git "${SOURCES[@]}" 2>/dev/null \
    | awk '{ s += $1 } END { print s + 0 }')"
fi
# Paczka git ≈ spakowane obiekty + luźne obiekty (`count-objects`, w KiB).
GIT_KB="$("${GIT[@]}" count-objects -v 2>/dev/null \
  | awk -F': ' '$1 == "size" || $1 == "size-pack" { s += $2 } END { print s + 0 }')"
EST_KB=$((SRC_KB + GIT_KB))
NEED_KB=$((2 * EST_KB))
echo "szacowany rozmiar kopii: $(human "$EST_KB") (pliki $(human "$SRC_KB") + historia git $(human "$GIT_KB"));" \
  "wymagane wolne miejsce (2 ×): $(human "$NEED_KB")"
[ "$FREE_KB" -ge "$NEED_KB" ] \
  || refuse "za mało miejsca na $DRIVE: wolne $(human "$FREE_KB"), potrzeba co najmniej $(human "$NEED_KB")"
echo "sprawdzenia: ok"
if [ "$DRY_RUN" -eq 1 ]; then
  echo "--dry-run: niczego nie utworzono ani nie zapisano"
  exit 0
fi

# --- Kopia ---
FAILED=() # kroki, które się nie udały — kopia jest wtedy niepełna (kod 3)
fail() { FAILED+=("$1"); echo "$1: BŁĄD${2:+ ($2)}"; } # <krok> [szczegóły]
STAMP="$(date +%Y-%m-%d_%H%M)"
mkdir -p "$BASE/kopia-$STAMP" || { echo "nie można utworzyć $BASE/kopia-$STAMP"; exit 1; }
# Ścieżka bezwzględna: `git -C "$ROOT"` liczyłby względny katalog docelowy od repozytorium,
# a `mkdir` i przekierowania — od bieżącego katalogu (paczka git trafiłaby w złe miejsce).
DST="$(cd "$BASE/kopia-$STAMP" && pwd)"
echo "kopia do: $DST"

# 1. Historia: paczka git (odtworzenie: git clone alfa.bundle alfa).
if "${GIT[@]}" bundle create "$DST/alfa.bundle" --all >/dev/null 2>"$DST/paczka-git.log" \
  && "${GIT[@]}" bundle verify "$DST/alfa.bundle" >>"$DST/paczka-git.log" 2>&1; then
  echo "paczka git: ok"
  rm -f "$DST/paczka-git.log"
else
  fail "paczka git" "szczegóły: $DST/paczka-git.log"
fi
"${GIT[@]}" log --oneline -1 >"$DST/ostatni-commit.txt" || fail "ostatni-commit.txt"
"${GIT[@]}" status --short >"$DST/niezatwierdzone-zmiany.txt" || fail "niezatwierdzone-zmiany.txt"

# 2. Źródła i katalogi projektu (robocopy: kody < 8 = sukces)
copy_dir() {
  local src="$1" name="$2"
  [ -d "$src" ] || return 0
  # MSYS_NO_PATHCONV: Git Bash zamieniał przełączniki /E, /R:1 na ścieżki (robocopy kod 16, 2026-10-16).
  MSYS_NO_PATHCONV=1 MSYS2_ARG_CONV_EXCL='*' robocopy "$(cygpath -w "$src")" "$(cygpath -w "$DST/$name")" \
    /E /R:1 /W:1 /XD target node_modules .git /NFL /NDL /NJH /NJS /NP >/dev/null
  local code=$?
  if [ "$code" -lt 8 ]; then echo "$name: ok"; else fail "$name" "robocopy $code"; fi
}
for entry in "${DIRS[@]}"; do copy_dir "${entry%%|*}" "${entry#*|}"; done
for f in "${FILES[@]}"; do
  [ ! -f "$f" ] || cp -f -- "$f" "$DST/" || fail "${f##*/}"
done

# 3. Sumy i instrukcja odtworzenia
(cd "$DST" && find . -type f ! -name SHA256SUMS.txt -print0 | sort -z | xargs -0 -r sha256sum >SHA256SUMS.txt) \
  || fail "SHA256SUMS.txt"
cat >"$DST/ODTWORZENIE.md" <<EOF || fail "ODTWORZENIE.md"
# Kopia bezpieczeństwa Alfy — $STAMP

- \`alfa.bundle\` — pełna historia repozytorium (wszystkie gałęzie):
  \`git clone alfa.bundle alfa\`, potem \`git checkout <gałąź>\` (ostatni commit: \`ostatni-commit.txt\`).
- \`alfa/\` — kopia katalogu roboczego bez \`target\` i \`node_modules\` (odtworzenie zależności:
  \`pnpm install\` i budowa według \`docs/KONTYNUACJA.md\`).
- \`Alfa-uruchomiona/\` — zbudowany program; uruchomienie: \`alfa-desktop.exe\`.
- \`memory/\`, \`.remember/\`, \`pamiec-claude/\` — pamięć projektu i notatki sesji.
- \`SHA256SUMS.txt\` — sumy kontrolne: \`sha256sum -c SHA256SUMS.txt\`.
EOF
du -sh "$DST" 2>/dev/null | tail -1
if [ "${#FAILED[@]}" -gt 0 ]; then
  echo "KOPIA NIEPEŁNA: nie udało się: ${FAILED[*]} — $DST (kod 3)"
  exit 3
fi
echo "gotowe: $DST"
