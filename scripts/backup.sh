#!/usr/bin/env bash
# Kopia bezpieczeństwa projektu na inny dysk (polecenie właściciela 2026-10-15: kopia na F:).
# Zawartość: paczka git z pełną historią (wszystkie gałęzie), źródła bez plików kompilacji
# (`target`, `node_modules`), zbudowany program (`Alfa-uruchomiona`), pamięć projektu, notatki
# sesji, plan i audyt, sumy SHA-256 i instrukcja odtworzenia. Niczego nie usuwa — każda kopia
# trafia do nowego katalogu z datą i godziną.
#
#   scripts/backup.sh [katalog_docelowy]      (domyślnie /f/Alfa)
set -uo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
PROJECT="$(cd "$ROOT/.." && pwd)"
BASE="${1:-/f/Alfa}"
STAMP="$(date +%Y-%m-%d_%H%M)"
DST="$BASE/kopia-$STAMP"
mkdir -p "$DST" || { echo "nie można utworzyć $DST"; exit 1; }
echo "kopia do: $DST"

# 1. Historia: paczka git (odtworzenie: git clone alfa.bundle alfa)
git -C "$ROOT" bundle create "$DST/alfa.bundle" --all >/dev/null 2>&1 && echo "paczka git: ok"
git -C "$ROOT" log --oneline -1 >"$DST/ostatni-commit.txt"
git -C "$ROOT" status --short >"$DST/niezatwierdzone-zmiany.txt"

# 2. Źródła i katalogi projektu (robocopy: kody < 8 = sukces)
copy_dir() {
  local src="$1" name="$2"
  [ -d "$src" ] || return 0
  # MSYS_NO_PATHCONV: Git Bash zamieniał przełączniki /E, /R:1 na ścieżki (robocopy kod 16, 2026-10-16).
  MSYS_NO_PATHCONV=1 MSYS2_ARG_CONV_EXCL='*' robocopy "$(cygpath -w "$src")" "$(cygpath -w "$DST/$name")" \
    /E /R:1 /W:1 /XD target node_modules .git /NFL /NDL /NJH /NJS /NP >/dev/null
  local code=$?
  if [ "$code" -lt 8 ]; then echo "$name: ok"; else echo "$name: BŁĄD (robocopy $code)"; fi
}
copy_dir "$ROOT" "alfa"
copy_dir "$PROJECT/Alfa-uruchomiona" "Alfa-uruchomiona"
copy_dir "$PROJECT/memory" "memory"
copy_dir "$PROJECT/.remember" ".remember"
copy_dir "$PROJECT/PLAN" "PLAN"
copy_dir "$PROJECT/audyt-2026-10-14" "audyt-2026-10-14"
for f in PLAN-KONTYNUACJI.md; do
  [ -f "$PROJECT/$f" ] && cp -f "$PROJECT/$f" "$DST/"
done
HOME_MEM="$(cygpath -u "$USERPROFILE")/.claude/projects/E--PROGRAMY-W-CZASIE-PRAC-Alfa-AI-Program/memory"
copy_dir "$HOME_MEM" "pamiec-claude"

# 3. Sumy i instrukcja odtworzenia
(cd "$DST" && find . -type f ! -name SHA256SUMS.txt -print0 | sort -z | xargs -0 sha256sum >SHA256SUMS.txt)
cat >"$DST/ODTWORZENIE.md" <<EOF
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
echo "gotowe: $DST"
