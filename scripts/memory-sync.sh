#!/usr/bin/env bash
# Kopia pamięci projektu do repozytorium: `docs/pamiec-projektu/` (zasada właściciela z 2026-10-12 —
# pamięć projektu i wszystko, co z nią związane, ma leżeć także w katalogu projektu, dobrze opisane).
# Źródła zostają tam, gdzie czytają je narzędzia; tu trafia ich kopia, więc wędruje z repozytorium.
#
#   scripts/memory-sync.sh            kopiuje nowe i zmienione pliki; plików, których nie ma u źródła, nie usuwa
#   scripts/memory-sync.sh --prune    usuwa też z kopii pliki, które celowo usunięto u źródła
#   scripts/memory-sync.sh --help     ten opis
#
# Plik, który jest tylko w kopii w repozytorium (zasady z drugiego komputera, pusta pamięć asystenta na nowej
# maszynie), zostaje na miejscu i trafia na listę OSTRZEŻEŃ z gotowym poleceniem przywrócenia go do źródła.
# Wyjątek: notatka dziennika `X.md`, którą wtyczka zamknęła u źródła jako `X.done.md`, znika z kopii od razu.
# Ochrona przed nową maszyną dotyczy tylko zasad pracy: gdy ich źródło wygląda na nowe (nie istnieje, nie ma
# w nim spisu MEMORY.md, który jest w kopii, albo brakuje co najmniej połowy plików kopii), pliki różniące się od źródła
# nie są nadpisywane, `--prune` niczego tam nie usuwa, a skrypt kończy się kodem 3. 2026-10-17/18 lustro
# skasowało z repozytorium zasady pracy, gdy dysk przeszedł na komputer z pustą pamięcią asystenta. Zasada,
# której treść u źródła jest wcześniejszą zatwierdzoną wersją pliku w kopii (stara pamięć drugiego komputera),
# też nie nadpisuje nowszej — 2026-10-21 PC cofnął tak w repozytorium zasady dwóch maszyn; kod 3. Tak samo
# zasada usunięta w repozytorium (zatwierdzone usunięcie), której plik u źródła jest starszy niż to usunięcie,
# nie wraca do kopii — skrypt podaje polecenie usunięcia jej u źródła; kod 3. Stan
# projektu i dziennik sesji są aktualizowane zawsze. Pliki są porównywane bez znaków CR (źródła mają CRLF,
# repozytorium LF), więc sama zmiana końców wierszy nie jest zmianą.
#
# Katalogi źródłowe:
#   ../memory/                                  stan projektu (status.md, handoff.json, decisions.md)
#   ~/.claude/projects/<projekt>/memory/        zasady pracy zapisane przez asystenta (ALFA_ASSISTANT_MEMORY)
#   ../.remember/*.md                           dziennik sesji (bez plików roboczych i logów)
# Sekrety nie mają prawa się tu znaleźć: plik źródłowy z czymś, co wygląda na klucz, nie trafia do kopii;
# plik z kluczem w kopii jest zastępowany czystą wersją ze źródła, a bez niej skrypt podaje polecenie usunięcia.
# Kody wyjścia: 0 — kopia zrobiona (także z ostrzeżeniami), 1 — coś jak klucz, 2 — zły argument,
# 3 — kopia niepełna: źródło zasad pracy wygląda na nowe albo ma starsze wersje zasad niż repozytorium
# (OSTATNIA-KOPIA.txt bez zmian).
set -uo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
PARENT="$(cd "$ROOT/.." && pwd)"
DST="$ROOT/docs/pamiec-projektu"
ASSISTANT="${ALFA_ASSISTANT_MEMORY:-$HOME/.claude/projects/E--PROGRAMY-W-CZASIE-PRAC-Alfa-AI-Program/memory}"
KEY_RE='sk-[A-Za-z0-9_-]{20,}|-----BEGIN [A-Z ]*PRIVATE KEY-----'
# git w katalogu repozytorium. `cd` zamiast `git -C`: Git Bash nie przekształca ścieżki z apostrofem
# w argumencie. `safe.directory`: dysk przeniesiony z innej instalacji Windows ma pliki innego właściciela (SID).
git_root() { (cd "$ROOT" && git -c safe.directory='*' "$@"); }

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
has_key() { grep -qE -- "$KEY_RE" "$1" 2>/dev/null; }
# Ta sama treść bez względu na końce wierszy (CRLF u źródła, LF po checkoucie repozytorium).
same_text() { cmp -s <(tr -d '\r' <"$1") <(tr -d '\r' <"$2"); }
copy_files() { find "$1" -maxdepth 1 -type f ! -name README.md -print0 | sort -z; }

RESTORE=()                # polecenia przywrócenia plików, które są tylko w kopii
REPO_ONLY=0               # ile takich plików
RULES_ONLY_IN_REPO=0      # 1 = któraś zasada pracy jest tylko w kopii (trzeba uzupełnić też spis MEMORY.md)
FRESH=""                  # katalog kopii, którego źródło wygląda na nowe (tylko zasady pracy)
HELD=()                   # polecenia porównania plików, których kopii nie nadpisano
STALE=()                  # polecenia przywrócenia nowszej zasady z kopii do źródła (u źródła starsza wersja)
DELETED=()                # polecenia usunięcia u źródła zasad usuniętych w repozytorium (u źródła kopia sprzed)
STALE_INDEX=""            # polecenie porównania spisu MEMORY.md wstrzymanego razem ze starszymi zasadami
PRUNE_RM=()               # pliki, których --prune nie usunął, bo źródło wygląda na nowe
KEY_SRC=()                # pliki źródłowe z czymś jak klucz (nie skopiowane)
declare -A NEEDS_MKDIR=() # brakujące katalogi źródłowe, dla których jest już `mkdir -p`

# Plik tylko w kopii: zapamiętuje polecenie przywrócenia (`cp -n` niczego u źródła nie nadpisze).
note_repo_only() { # <źródło> <plik w kopii>
  local src="$1" file="$2"
  has_key "$file" && return 0 # takiego pliku nie przywracamy — obsługuje go sprawdzenie sekretów
  if [ ! -d "$src" ] && [ -z "${NEEDS_MKDIR[$src]:-}" ]; then
    RESTORE+=("mkdir -p $(shq "$src")")
    NEEDS_MKDIR[$src]=1
  fi
  RESTORE+=("cp -n $(shq "$file") $(shq "$src/")")
  REPO_ONLY=$((REPO_ONLY + 1))
  [ "$src" != "$ASSISTANT" ] || RULES_ONLY_IN_REPO=1
}

# Źródło wygląda na nowe (nowa maszyna, pusta pamięć asystenta): nie istnieje, nie ma w nim spisu MEMORY.md,
# który jest w kopii, albo brakuje co najmniej połowy plików kopii. Pojedyncze pliki usunięte celowo tego
# nie wywołują (brak katalogu źródłowego sprawdza `copy_dir`).
looks_fresh() { # <źródło> <cel>
  local src="$1" dst="$2" f total=0 missing=0
  if [ -f "$dst/MEMORY.md" ] && [ ! -f "$src/MEMORY.md" ]; then return 0; fi
  while IFS= read -r -d '' f; do
    total=$((total + 1))
    [ -f "$src/${f##*/}" ] || missing=$((missing + 1))
  done < <(copy_files "$dst")
  [ "$total" -gt 0 ] && [ $((2 * missing)) -ge "$total" ]
}

# Plik źródłowy to stara pamięć drugiego komputera: jego treść to któraś wcześniej zatwierdzona wersja
# pliku kopii (inna niż obecna kopia, także niezatwierdzona), a zmieniono go u źródła przed ostatnim
# commitem tego pliku w repozytorium. Celowy powrót do dawnej treści ma świeżą datę zmiany i przechodzi.
# Bez repozytorium git nie da się tego stwierdzić (`1`).
older_version() { # <plik źródłowy> <plik kopii względem repozytorium>
  local src="$1" rel="$2" commit last mtime
  git_root rev-parse --is-inside-work-tree >/dev/null 2>&1 || return 1
  last="$(git_root log -1 --format=%ct -- "$rel" 2>/dev/null)"
  mtime="$(stat -c %Y -- "$src" 2>/dev/null)"
  { [ -n "$last" ] && [ -n "$mtime" ] && [ "$mtime" -lt "$last" ]; } || return 1
  while IFS= read -r commit; do
    cmp -s <(git_root show "$commit:$rel" 2>/dev/null | tr -d '\r') <(tr -d '\r' <"$ROOT/$rel") && continue
    cmp -s <(tr -d '\r' <"$src") <(git_root show "$commit:$rel" 2>/dev/null | tr -d '\r') && return 0
  done < <(git_root log --format=%H -- "$rel" 2>/dev/null)
  return 1
}

# Zasada usunięta w repozytorium (zatwierdzone usunięcie), a u źródła plik sprzed usunięcia — pamięć drugiego
# komputera. Bez tego usunięta zasada wracała przy pierwszej synchronizacji na tamtej maszynie (inwentaryzacja 21.10).
deleted_in_repo() { # <plik źródłowy> <plik kopii względem repozytorium>
  local src="$1" rel="$2" gone mtime commit
  git_root rev-parse --is-inside-work-tree >/dev/null 2>&1 || return 1
  gone="$(git_root log -1 --diff-filter=D --format=%ct -- "$rel" 2>/dev/null)"
  mtime="$(stat -c %Y -- "$src" 2>/dev/null)"
  { [ -n "$gone" ] && [ -n "$mtime" ] && [ "$mtime" -lt "$gone" ]; } || return 1
  # Tylko treść, którą repozytorium kiedyś zatwierdziło. Inna (zmiana, która nie zdążyła się zsynchronizować)
  # to jedyna kopia — wraca do repozytorium jak nowa zasada, a nie do usunięcia (przegląd 22.10).
  while IFS= read -r commit; do
    cmp -s <(tr -d '\r' <"$src") <(git_root show "$commit:$rel" 2>/dev/null | tr -d '\r') && return 0
  done < <(git_root log --format=%H -- "$rel" 2>/dev/null)
  return 1
}

# Spis MEMORY.md na końcu: wstrzymywany, gdy któraś zasada u źródła okazała się starsza (STALE).
memory_last() {
  local f memo="" list=()
  while IFS= read -r -d '' f; do
    if [ "${f##*/}" = MEMORY.md ]; then memo="$f"; else list+=("$f"); fi
  done
  [ -z "$memo" ] || list+=("$memo")
  [ "${#list[@]}" -eq 0 ] || printf '%s\0' "${list[@]}"
}

copy_dir() { # <źródło> <cel> <wzorzec> [1 = chronić kopię przed źródłem, które wygląda na nowe]
  local src="$1" dst="$2" pattern="$3" guard="${4:-0}" f name rel="${2#"$ROOT"/}" fresh=0 had_key
  if [ ! -d "$src" ]; then
    # Brak źródła to typowy objaw nowej maszyny — kopia zostaje nietknięta także z `--prune`.
    echo "pominięto (brak źródła): $src"
    [ -d "$dst" ] || return 0
    while IFS= read -r -d '' f; do
      note_repo_only "$src" "$f"
      [ "$guard" -eq 0 ] || FRESH="$rel"
    done < <(copy_files "$dst")
    return 0
  fi
  mkdir -p "$dst"
  if [ "$guard" -eq 1 ] && looks_fresh "$src" "$dst"; then
    fresh=1
    FRESH="$rel"
  fi
  # 1. Pliki, których u źródła nie ma: zostają (ostrzeżenie), z `--prune` znikają — chyba że źródło
  #    wygląda na nowe. Notatka zamknięta u źródła (`X.md` → `X.done.md`) znika zawsze.
  while IFS= read -r -d '' f; do
    name="${f##*/}"
    if [ -f "$src/$name" ] && [[ "$name" == $pattern ]]; then continue; fi
    if [[ "$name" == *.md && "$name" != *.done.md ]] && [ -f "$src/${name%.md}.done.md" ]; then
      rm -f -- "$f" && echo "zamknięty u źródła jako ${name%.md}.done.md — usunięto z kopii: $rel/$name"
    elif [ "$PRUNE" -eq 1 ] && [ "$fresh" -eq 0 ]; then
      rm -f -- "$f" && echo "usunięto z kopii (--prune): $rel/$name"
    elif [ ! -f "$src/$name" ]; then
      note_repo_only "$src" "$f"
      [ "$PRUNE" -eq 0 ] || PRUNE_RM+=("rm -- $(shq "$f")")
    fi
  done < <(copy_files "$dst")
  # 2. Nowe i zmienione pliki ze źródła. Plik z czymś jak klucz nie trafia do kopii; plik z kluczem w kopii
  #    jest nadpisywany czystą wersją ze źródła także wtedy, gdy źródło wygląda na nowe.
  while IFS= read -r -d '' f; do
    name="${f##*/}"
    if has_key "$f"; then
      KEY_SRC+=("$f")
      continue
    fi
    if [ ! -e "$dst/$name" ]; then
      if [ "$guard" -eq 1 ] && deleted_in_repo "$f" "$rel/$name"; then
        DELETED+=("rm -- $(shq "$f")")
        continue
      fi
      cp -f -- "$f" "$dst/" && echo "nowy: $rel/$name"
      continue
    fi
    same_text "$f" "$dst/$name" && continue
    had_key=0
    has_key "$dst/$name" && had_key=1
    if [ "$fresh" -eq 1 ] && [ "$had_key" -eq 0 ]; then
      HELD+=("diff -u --strip-trailing-cr $(shq "$dst/$name") $(shq "$f")")
    elif [ "$guard" -eq 1 ] && [ "$had_key" -eq 0 ] && older_version "$f" "$rel/$name"; then
      STALE+=("cp $(shq "$dst/$name") $(shq "$f")")
    elif [ "$guard" -eq 1 ] && [ "$had_key" -eq 0 ] && [ "$name" = MEMORY.md ] && [ $((${#STALE[@]} + ${#DELETED[@]})) -gt 0 ]; then
      STALE_INDEX="diff -u --strip-trailing-cr $(shq "$dst/$name") $(shq "$f")"
    elif cp -f -- "$f" "$dst/"; then
      echo "zaktualizowany: $rel/$name"
      [ "$had_key" -eq 0 ] \
        || echo "  (w poprzedniej wersji było coś jak klucz — jeśli była zatwierdzona, unieważnij ten klucz)"
    fi
  done < <(find "$src" -maxdepth 1 -type f -name "$pattern" -print0 | sort -z | memory_last)
}

# Zatwierdzona wersja pliku istnieje i nie ma w niej nic jak klucz.
committed_clean() { # <ścieżka względem repozytorium>
  local content
  git_root cat-file -e "HEAD:$1" 2>/dev/null || return 1
  content="$(git_root show "HEAD:$1" 2>/dev/null)" || return 1
  ! grep -qE -- "$KEY_RE" <<<"$content"
}

copy_dir "$PARENT/memory" "$DST/1-stan-projektu" "*"
copy_dir "$ASSISTANT" "$DST/2-zasady-pracy" "*.md" 1
copy_dir "$PARENT/.remember" "$DST/3-dziennik-sesji" "*.md"

# Pliki z kluczem, które zostały w kopii (np. po wcześniejszych wersjach skryptu): polecenie usunięcia.
KEY_FIX=()
while IFS= read -r -d '' f; do
  rel="${f#"$ROOT"/}"
  if committed_clean "$rel"; then
    KEY_FIX+=("(cd $(shq "$ROOT") && git checkout HEAD -- $(shq "$rel"))")
  else
    KEY_FIX+=("rm -- $(shq "$f")")
  fi
done < <(grep -rlZE -- "$KEY_RE" "$DST" 2>/dev/null | sort -z)
KEY_FOUND=0
[ "${#KEY_SRC[@]}" -eq 0 ] && [ "${#KEY_FIX[@]}" -eq 0 ] || KEY_FOUND=1

if [ "$KEY_FOUND" -eq 0 ] && [ -z "$FRESH" ] && [ "${#STALE[@]}" -eq 0 ] && [ "${#DELETED[@]}" -eq 0 ]; then
  date +"%Y-%m-%d %H:%M" >"$DST/OSTATNIA-KOPIA.txt"
  echo "pamięć projektu skopiowana do $DST"
fi

if [ "$REPO_ONLY" -gt 0 ]; then
  echo
  echo "OSTRZEŻENIE: $REPO_ONLY plik(ów) jest tylko w kopii w repozytorium — u źródła ich nie ma, więc NIE"
  echo "zostały usunięte. Jeśli mają zostać (np. zasady z drugiego komputera), przywróć je do źródła:"
  printf '  %s\n' "${RESTORE[@]}"
  if [ "$RULES_ONLY_IN_REPO" -eq 1 ] && [ -n "$FRESH" ]; then
    echo "Po przywróceniu zasad połącz u źródła spis MEMORY.md z wersją z kopii w repozytorium."
  elif [ "$RULES_ONLY_IN_REPO" -eq 1 ]; then
    echo "Po przywróceniu zasad dopisz je u źródła do spisu MEMORY.md (wcześniejsze wersje spisu:"
    echo "  git log -p -- docs/pamiec-projektu/2-zasady-pracy/MEMORY.md)."
  fi
  echo "Potem uruchom ten skrypt ponownie."
  [ -n "$FRESH" ] \
    || echo "Jeśli usunięto je u źródła celowo: bash scripts/memory-sync.sh --prune (usuwa je z kopii)."
fi

if [ -n "$FRESH" ]; then
  echo
  echo "KOPIA NIEPEŁNA: źródło zasad pracy ($ASSISTANT) wygląda na nowe — nie istnieje,"
  echo "nie ma w nim spisu MEMORY.md albo brakuje w nim co najmniej połowy plików z $FRESH"
  echo "(typowe na nowym komputerze)."
  echo "Najpierw przywróć zasady poleceniami wyżej, potem uruchom skrypt ponownie."
  if [ "${#HELD[@]}" -gt 0 ]; then
    echo "Nie nadpisano ${#HELD[@]} plik(ów) różniących się od źródła (kopia w repozytorium → źródło):"
    printf '  %s\n' "${HELD[@]}"
  fi
  if [ "${#PRUNE_RM[@]}" -gt 0 ]; then
    echo "--prune niczego z $FRESH nie usunął. Jeśli te pliki naprawdę usunięto u źródła celowo,"
    echo "usuń je z kopii ręcznie:"
    printf '  %s\n' "${PRUNE_RM[@]}"
  fi
fi

if [ "${#STALE[@]}" -gt 0 ]; then
  echo
  echo "STARSZE ZASADY U ŹRÓDŁA: ${#STALE[@]} plik(ów) w $ASSISTANT ma treść wcześniejszej wersji"
  echo "z repozytorium (pamięć z drugiego komputera) — nowszej kopii nie nadpisano. Przywróć ją do źródła,"
  echo "uzupełnij u źródła spis MEMORY.md (git log -p -- docs/pamiec-projektu/2-zasady-pracy/MEMORY.md)"
  echo "i uruchom skrypt ponownie:"
  printf '  %s\n' "${STALE[@]}"
  if [ -n "$STALE_INDEX" ]; then
    echo "Spisu MEMORY.md też nie nadpisano — połącz go u źródła z kopią (pozycje z obu):"
    echo "  $STALE_INDEX"
  fi
  echo "Jeśli treść u źródła jest celowo dawną wersją, zapisz plik u źródła ponownie (nowa data zmiany)."
fi

if [ "${#DELETED[@]}" -gt 0 ]; then
  echo
  echo "ZASADY USUNIĘTE W REPOZYTORIUM: ${#DELETED[@]} plik(ów) w $ASSISTANT jest sprzed ich usunięcia"
  echo "w repozytorium (pamięć z drugiego komputera) — nie wróciły do kopii. Usuń je u źródła, popraw u źródła"
  echo "spis MEMORY.md i uruchom skrypt ponownie:"
  printf '  %s\n' "${DELETED[@]}"
  if [ -n "$STALE_INDEX" ] && [ "${#STALE[@]}" -eq 0 ]; then
    echo "Spisu MEMORY.md nie nadpisano — porównaj go u źródła z kopią:"
    echo "  $STALE_INDEX"
  fi
  echo "Jeśli zasada ma wrócić, zapisz plik u źródła ponownie (nowa data zmiany)."
fi

if [ "$KEY_FOUND" -eq 1 ]; then
  echo
  echo "BŁĄD: coś, co wygląda na klucz (sk-… albo klucz prywatny)."
  if [ "${#KEY_SRC[@]}" -gt 0 ]; then
    echo "U źródła — tych plików nie skopiowano; usuń z nich klucz (albo cały plik) i uruchom skrypt ponownie:"
    printf '  %s\n' "${KEY_SRC[@]}"
  fi
  if [ "${#KEY_FIX[@]}" -gt 0 ]; then
    echo "W kopii w repozytorium — usuń je stąd, zanim cokolwiek zatwierdzisz (klucz, który był już"
    echo "zatwierdzony, jest w historii git — unieważnij go):"
    printf '  %s\n' "${KEY_FIX[@]}"
  fi
fi

if git_root rev-parse --is-inside-work-tree >/dev/null 2>&1; then
  git_root status --short -- docs/pamiec-projektu | head -20
fi

# Jeden wiersz podsumowania na końcu — to widać po zakończeniu sesji.
echo
if [ "$KEY_FOUND" -eq 1 ]; then
  echo "WYNIK: BŁĄD — coś jak klucz (kod 1), OSTATNIA-KOPIA.txt bez zmian."
  exit 1
elif [ -n "$FRESH" ]; then
  echo "WYNIK: KOPIA NIEPEŁNA — wstrzymano ${#HELD[@]} plik(ów), $REPO_ONLY plik(ów) tylko w kopii (kod 3)," \
    "OSTATNIA-KOPIA.txt bez zmian."
  exit 3
elif [ "${#STALE[@]}" -gt 0 ]; then
  echo "WYNIK: KOPIA NIEPEŁNA — ${#STALE[@]} zasad(y) u źródła starsze niż w repozytorium (kod 3)," \
    "OSTATNIA-KOPIA.txt bez zmian."
  exit 3
elif [ "${#DELETED[@]}" -gt 0 ]; then
  echo "WYNIK: KOPIA NIEPEŁNA — ${#DELETED[@]} zasad(y) usuniętych w repozytorium jest jeszcze u źródła (kod 3)," \
    "OSTATNIA-KOPIA.txt bez zmian."
  exit 3
elif [ "$REPO_ONLY" -gt 0 ]; then
  echo "WYNIK: kopia zrobiona; $REPO_ONLY plik(ów) jest tylko w kopii — patrz OSTRZEŻENIE wyżej."
else
  echo "WYNIK: kopia zrobiona."
fi
