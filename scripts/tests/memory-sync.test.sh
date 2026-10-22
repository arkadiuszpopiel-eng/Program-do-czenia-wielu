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
REMEMBER="$SB/projekt/.remember"
COPY="$SB/projekt/alfa/docs/pamiec-projektu"
RULES="$COPY/2-zasady-pracy"
JOURNAL="$COPY/3-dziennik-sesji"
KEY="sk-$(printf 'x%.0s' {1..24})"

setup() { # świeża piaskownica: projekt/alfa (repozytorium), projekt/memory, projekt/.remember, asystent
  rm -rf "$SB"
  mkdir -p "$SB/projekt/alfa/scripts" "$SB/projekt/memory" "$REMEMBER" "$ASSISTANT" \
    "$COPY/1-stan-projektu" "$RULES" "$JOURNAL"
  cp "$HERE/../memory-sync.sh" "$SB/projekt/alfa/scripts/"
  echo "opis kopii" >"$COPY/README.md"
  echo "stan" >"$SB/projekt/memory/status.md"
  echo "dziennik" >"$REMEMBER/today.md"
}
rules() { # <katalog> <nazwy…> — zasady i spis MEMORY.md, który na nie wskazuje
  local dir="$1" n
  shift
  : >"$dir/MEMORY.md"
  for n in "$@"; do
    echo "zasada $n" >"$dir/$n.md"
    echo "- [$n]($n.md)" >>"$dir/MEMORY.md"
  done
}
sync() { run env ALFA_ASSISTANT_MEMORY="$ASSISTANT" bash "$SB/projekt/alfa/scripts/memory-sync.sh" "$@"; }
# Wykonuje polecenia z wyjścia, które zaczynają się od podanego przedrostka (np. „  cp -n ”).
run_listed() { # <przedrostek…>
  local line prefix
  while IFS= read -r line; do
    for prefix in "$@"; do
      if [[ "$line" == "$prefix"* ]]; then eval "$line" || return 1; fi
    done
  done <<<"$OUT"
}
restore_from_warning() { run_listed "  mkdir -p " "  cp -n "; }
same() { cmp -s "$1" "$2"; }
no_key_in_copy() { ! grep -rqE "sk-x{20,}" "$COPY"; }

echo "memory-sync.sh"

setup
echo "zasada A" >"$ASSISTANT/a.md"
echo "notatka" >"$ASSISTANT/szkic.txt"
sync
check "kod 0 przy zwykłej kopii" rc_is 0
check "nowy plik ze źródła trafia do kopii" same "$ASSISTANT/a.md" "$RULES/a.md"
check "plik spoza wzorca (*.md) nie trafia do kopii" test ! -e "$RULES/szkic.txt"
check "stan projektu i dziennik skopiowane" test -f "$COPY/1-stan-projektu/status.md" -a -f "$JOURNAL/today.md"
check "data kopii zapisana" test -s "$COPY/OSTATNIA-KOPIA.txt"
check "bez plików tylko w kopii — bez ostrzeżenia" eval '! out_has OSTRZEŻENIE'
check "podsumowanie w ostatnim wierszu" eval '[[ "$(tail -1 <<<"$OUT")" == "WYNIK: kopia zrobiona." ]]'
echo "zasada A, wersja 2" >"$ASSISTANT/a.md"
sync
check "zmieniony plik jest nadpisany" same "$ASSISTANT/a.md" "$RULES/a.md"
check "wypisuje zaktualizowany plik" out_has "zaktualizowany: docs/pamiec-projektu/2-zasady-pracy/a.md"
printf 'zasada A, wersja 2\r\n' >"$ASSISTANT/a.md"
sync
check "CRLF u źródła i LF w kopii to ta sama treść" eval '! out_has "zaktualizowany"'

# Wtyczka dziennika zamyka dzień: today-RRRR-MM-DD.md → today-RRRR-MM-DD.done.md.
setup
echo "dzień" >"$REMEMBER/today-2026-10-18.md"
echo "teraz" >"$REMEMBER/now.md"
rules "$ASSISTANT" a b c
sync
mv "$REMEMBER/today-2026-10-18.md" "$REMEMBER/today-2026-10-18.done.md"
echo "teraz, wersja 2" >"$REMEMBER/now.md"
sync
check ".done.md: kod 0" rc_is 0
check ".done.md: zamknięta notatka znika z kopii" test ! -e "$JOURNAL/today-2026-10-18.md"
check ".done.md: zamknięta wersja trafia do kopii" test -f "$JOURNAL/today-2026-10-18.done.md"
check ".done.md: inne pliki dziennika aktualizowane" same "$REMEMBER/now.md" "$JOURNAL/now.md"
check ".done.md: bez ostrzeżenia i bez polecenia przywrócenia" eval '! out_has OSTRZEŻENIE && ! out_has "cp -n"'
check ".done.md: data kopii zapisana" grep -q . "$COPY/OSTATNIA-KOPIA.txt"

# Zasada usunięta (scalona) u źródła celowo, spis MEMORY.md poprawiony.
setup
rules "$ASSISTANT" a b c d e
sync
rules "$ASSISTANT" a b c d
rm "$ASSISTANT/e.md"
echo "zasada a, poprawiona" >"$ASSISTANT/a.md"
sync
check "usunięcie u źródła: kod 0" rc_is 0
check "usunięcie u źródła: spis MEMORY.md zaktualizowany" same "$ASSISTANT/MEMORY.md" "$RULES/MEMORY.md"
check "usunięcie u źródła: zmieniona zasada zaktualizowana" same "$ASSISTANT/a.md" "$RULES/a.md"
check "usunięcie u źródła: plik zostaje w kopii z ostrzeżeniem" eval 'test -f "$RULES/e.md" && out_has "OSTRZEŻENIE: 1 plik(ów)"'
check "usunięcie u źródła: podpowiedź --prune" out_has "memory-sync.sh --prune"
check "usunięcie u źródła: to nie jest kopia niepełna" eval '! out_has "KOPIA NIEPEŁNA"'
sync --prune
check "--prune: kod 0" rc_is 0
check "--prune: plik usunięty z kopii" test ! -e "$RULES/e.md"
check "--prune: wypisuje usunięty plik" out_has "usunięto z kopii (--prune): docs/pamiec-projektu/2-zasady-pracy/e.md"
check "--prune: README.md w kopii zostaje" test -f "$COPY/README.md"

# Nowy komputer: pamięć asystenta prawie pusta, a w kopii zasady z poprzedniej maszyny.
setup
rules "$RULES" r1 r2 r3 r4 r5
rules "$ASSISTANT" nowa
echo "stan, wersja 2" >"$SB/projekt/memory/status.md"
echo "stary" >"$COPY/1-stan-projektu/stary.md"
echo "dziennik, wersja 2" >"$REMEMBER/today.md"
echo "dziennik" >"$JOURNAL/today.md"
sync
check "nowa maszyna: kod 3 (kopia niepełna)" rc_is 3
check "nowa maszyna: podsumowanie KOPIA NIEPEŁNA" eval '[[ "$(tail -1 <<<"$OUT")" == "WYNIK: KOPIA NIEPEŁNA"* ]]'
check "nowa maszyna: data kopii nie zapisana" test ! -e "$COPY/OSTATNIA-KOPIA.txt"
check "nowa maszyna: zasady w kopii zostają" test -f "$RULES/r1.md" -a -f "$RULES/r5.md"
check "nowa maszyna: spis MEMORY.md nie jest nadpisany" grep -q "r5.md" "$RULES/MEMORY.md"
check "nowa maszyna: polecenie porównania spisu" out_has "diff -u --strip-trailing-cr"
check "nowa maszyna: nowa zasada i tak trafia do kopii" same "$ASSISTANT/nowa.md" "$RULES/nowa.md"
check "nowa maszyna: stan projektu aktualizowany mimo pliku tylko w kopii" \
  same "$SB/projekt/memory/status.md" "$COPY/1-stan-projektu/status.md"
check "nowa maszyna: dziennik aktualizowany" same "$REMEMBER/today.md" "$JOURNAL/today.md"
check "nowa maszyna: polecenie przywrócenia do pamięci asystenta" out_has "/asystent/'"
check "nowa maszyna: przypomnienie o połączeniu spisu MEMORY.md" out_has "spis MEMORY.md"
sync --prune
check "nowa maszyna, --prune: kod 3" rc_is 3
check "nowa maszyna, --prune: zasady w kopii zostają" test -f "$RULES/r1.md" -a -f "$RULES/r5.md"
check "nowa maszyna, --prune: wyjaśnia, że niczego nie usunął" out_has "--prune niczego z docs/pamiec-projektu/2-zasady-pracy nie usunął"
check "nowa maszyna, --prune: inne katalogi przycinane" test ! -e "$COPY/1-stan-projektu/stary.md"
sync
check "nowa maszyna: polecenie przywrócenia dla r1.md" out_has "r1.md' '"
echo "zasada r1 z nowej maszyny" >"$ASSISTANT/r1.md" # powstała u źródła po wypisaniu poleceń
check "polecenia z ostrzeżenia wykonują się" restore_from_warning
check "polecenie z ostrzeżenia przywraca plik do źródła" same "$RULES/r2.md" "$ASSISTANT/r2.md"
check "cp -n nie nadpisuje pliku, który już jest u źródła" grep -q "z nowej maszyny" "$ASSISTANT/r1.md"
rules "$ASSISTANT" nowa r1 r2 r3 r4 r5
sync
check "po przywróceniu i połączeniu spisu: kod 0" rc_is 0
check "po przywróceniu: bez ostrzeżenia" eval '! out_has OSTRZEŻENIE'
check "po przywróceniu: spis MEMORY.md zaktualizowany" same "$ASSISTANT/MEMORY.md" "$RULES/MEMORY.md"

setup
rules "$RULES" r1 r2
rm -rf "$ASSISTANT"
sync
check "brak katalogu źródłowego: kod 3 (kopia niepełna)" eval 'rc_is 3 && out_has "KOPIA NIEPEŁNA"'
check "brak katalogu źródłowego: kopia nietknięta" test -f "$RULES/r1.md" -a -f "$RULES/r2.md"
check "brak katalogu źródłowego: polecenie mkdir -p" out_has "mkdir -p '"
check "brak katalogu źródłowego: polecenia odtwarzają katalog" restore_from_warning
check "brak katalogu źródłowego: pliki przywrócone" same "$RULES/r1.md" "$ASSISTANT/r1.md"
check "mkdir -p wypisany raz na katalog" test "$(grep -c '^  mkdir -p' <<<"$OUT")" -eq 1
rm -rf "$ASSISTANT"
sync --prune
check "--prune przy braku źródła niczego nie usuwa" test -f "$RULES/r1.md" -a -f "$RULES/r2.md"

# Klucz u źródła nie trafia do kopii; po usunięciu klucza (albo pliku) kopia przechodzi.
setup
rules "$ASSISTANT" a b c
echo "inna zasada" >"$RULES/z-pc.md"
sync
printf 'klucz: %s\n' "$KEY" >>"$ASSISTANT/b.md"
printf 'klucz: %s\n' "$KEY" >"$ASSISTANT/notatka.md"
sync
check "klucz u źródła: kod 1" rc_is 1
check "klucz u źródła: komunikat BŁĄD" out_has "BŁĄD: coś, co wygląda na klucz"
check "klucz u źródła: wskazuje plik źródłowy" out_has "/asystent/notatka.md"
check "klucz u źródła: nie trafia do kopii" no_key_in_copy
check "klucz u źródła: bez polecenia przywrócenia pliku z kluczem" eval '! grep -q notatka <<<"$(grep -F "cp -n" <<<"$OUT")"'
echo "zasada b" >"$ASSISTANT/b.md"
rm "$ASSISTANT/notatka.md"
sync
check "klucz usunięty u źródła: kod 0" rc_is 0
check "klucz usunięty u źródła: kopia bez klucza" no_key_in_copy
# Klucz w kopii (z wcześniejszej wersji skryptu): czysta wersja ze źródła go zastępuje.
printf 'klucz: %s\n' "$KEY" >>"$RULES/a.md"
printf 'klucz: %s\n' "$KEY" >"$RULES/stara.md"
sync
check "klucz w kopii: kod 1, gdy pliku nie ma u źródła" rc_is 1
check "klucz w kopii: czysta wersja ze źródła zastępuje plik" same "$ASSISTANT/a.md" "$RULES/a.md"
check "klucz w kopii bez źródła: polecenie usunięcia" out_has "rm -- '"
check "polecenie usunięcia działa" run_listed "  rm -- "
sync
check "klucz usunięty z kopii: kod 0" eval 'rc_is 0 && no_key_in_copy'
# Klucz w zatwierdzonym pliku kopii, którego nie ma u źródła: polecenie przywraca czystą wersję z HEAD.
if command -v git >/dev/null; then
  setup
  rules "$ASSISTANT" a b c
  sync
  (cd "$SB/projekt/alfa" && git init -q && git config core.autocrlf false && git add -A \
    && git -c user.name=test -c user.email=test@example.invalid commit -qm kopia)
  rm "$ASSISTANT/c.md"
  printf 'klucz: %s\n' "$KEY" >>"$RULES/c.md"
  sync
  check "klucz w zatwierdzonym pliku: kod 1 i polecenie checkout HEAD" \
    eval 'rc_is 1 && out_has "checkout HEAD -- '"'"'docs/pamiec-projektu/2-zasady-pracy/c.md'"'"'"'
  check "polecenie checkout HEAD działa" run_listed "  (cd "
  sync
  check "po checkout HEAD: kod 0, kopia bez klucza" eval 'rc_is 0 && no_key_in_copy'
fi
# Pamięć drugiego komputera: zasada u źródła to wcześniejsza zatwierdzona wersja z repozytorium
# (2026-10-21 PC cofnął tak zasady dwóch maszyn) — nowsza kopia zostaje, kod 3, polecenie przywrócenia.
if command -v git >/dev/null; then
  setup
  rules "$ASSISTANT" a b c
  echo "zasada a, wersja 1" >"$ASSISTANT/a.md"
  sync
  commit_copy() { (cd "$SB/projekt/alfa" && git add -A && git -c user.name=test -c user.email=test@example.invalid commit -qm "$1"); }
  (cd "$SB/projekt/alfa" && git init -q && git config core.autocrlf false)
  commit_copy "wersja 1"
  echo "zasada a, wersja 2 (laptop)" >"$RULES/a.md"
  commit_copy "wersja 2"
  # Stara pamięć: treść wersji 1 i data zmiany sprzed commitu wersji 2; spis u źródła — stary plus nowa pozycja.
  printf 'zasada a, wersja 1\r\n' >"$ASSISTANT/a.md"
  touch -d '2026-01-01 00:00' "$ASSISTANT/a.md"
  echo "zasada b, nowa" >"$ASSISTANT/b.md"
  echo "- [nowa](nowa.md)" >>"$ASSISTANT/MEMORY.md"
  echo "zasada nowa" >"$ASSISTANT/nowa.md"
  rm -f "$COPY/OSTATNIA-KOPIA.txt"
  sync
  check "starsza zasada u źródła: kod 3" eval 'rc_is 3 && out_has "STARSZE ZASADY U ŹRÓDŁA"'
  check "starsza zasada u źródła: nowsza kopia zostaje" grep -q "wersja 2" "$RULES/a.md"
  check "starsza zasada u źródła: inne zmiany i tak trafiają do kopii" same "$ASSISTANT/b.md" "$RULES/b.md"
  check "starsza zasada u źródła: spis MEMORY.md wstrzymany" eval '! grep -q nowa.md "$RULES/MEMORY.md" && out_has "Spisu MEMORY.md też nie nadpisano"'
  check "starsza zasada u źródła: data kopii nie zapisana" test ! -e "$COPY/OSTATNIA-KOPIA.txt"
  check "starsza zasada u źródła: polecenie przywrócenia działa" run_listed "  cp "
  check "po przywróceniu: źródło ma nowszą wersję" grep -q "wersja 2" "$ASSISTANT/a.md"
  sync
  check "po przywróceniu: kod 0, spis zaktualizowany" eval 'rc_is 0 && grep -q nowa.md "$RULES/MEMORY.md"'
  echo "zasada a, wersja 3" >"$ASSISTANT/a.md"
  sync
  check "nowa wersja u źródła: nadpisuje kopię" eval 'rc_is 0 && same "$ASSISTANT/a.md" "$RULES/a.md"'
  # Celowy powrót do dawnej treści (świeża data zmiany) przechodzi.
  commit_copy "wersja 3"
  echo "zasada a, wersja 1" >"$ASSISTANT/a.md"
  sync
  check "celowy powrót do wersji 1: kopia nadpisana, kod 0" eval 'rc_is 0 && same "$ASSISTANT/a.md" "$RULES/a.md"'
  # Kopia z niezatwierdzoną nowszą treścią, u źródła stara wersja z HEAD — też wstrzymana.
  commit_copy "wersja 1 znów"
  echo "zasada a, wersja 4 (niezatwierdzona)" >"$RULES/a.md"
  echo "zasada a, wersja 1" >"$ASSISTANT/a.md"
  touch -d '2026-01-01 00:00' "$ASSISTANT/a.md"
  sync
  check "niezatwierdzona nowsza kopia: nie nadpisana, kod 3" eval 'rc_is 3 && grep -q "wersja 4" "$RULES/a.md"'
  # Zasada usunięta w repozytorium wraca ze starej pamięci drugiego komputera — nie powinna (inwentaryzacja 21.10).
  echo "zasada a, wersja 5" >"$ASSISTANT/a.md"
  echo "zasada z" >"$ASSISTANT/z.md"
  echo "- [z](z.md)" >>"$ASSISTANT/MEMORY.md"
  sync
  check "nowa zasada z: skopiowana, kod 0" eval 'rc_is 0 && test -f "$RULES/z.md"'
  commit_copy "z dodana"
  rm -f "$RULES/z.md"
  grep -v "z.md" "$RULES/MEMORY.md" >"$RULES/MEMORY.tmp" && mv "$RULES/MEMORY.tmp" "$RULES/MEMORY.md"
  commit_copy "z usunięta"
  touch -d '2026-01-01 00:00' "$ASSISTANT/z.md" "$ASSISTANT/MEMORY.md"
  rm -f "$COPY/OSTATNIA-KOPIA.txt"
  sync
  check "usunięta w repozytorium: nie wraca, kod 3" eval 'rc_is 3 && test ! -e "$RULES/z.md" && out_has "ZASADY USUNIĘTE W REPOZYTORIUM"'
  check "usunięta w repozytorium: spis MEMORY.md wstrzymany" eval '! grep -q z.md "$RULES/MEMORY.md"'
  check "usunięta w repozytorium: data kopii nie zapisana" test ! -e "$COPY/OSTATNIA-KOPIA.txt"
  check "usunięta w repozytorium: polecenie usunięcia u źródła działa" run_listed "  rm -- "
  grep -v "z.md" "$ASSISTANT/MEMORY.md" >"$ASSISTANT/MEMORY.tmp" && mv "$ASSISTANT/MEMORY.tmp" "$ASSISTANT/MEMORY.md"
  sync
  check "po usunięciu u źródła: kod 0" eval 'rc_is 0 && test ! -e "$RULES/z.md"'
  # Zasada odtworzona celowo (nowa data zmiany) wraca.
  echo "zasada z, odtworzona" >"$ASSISTANT/z.md"
  sync
  check "odtworzona celowo: wraca do kopii" eval 'rc_is 0 && same "$ASSISTANT/z.md" "$RULES/z.md"'
  # Treść, której repozytorium nigdy nie widziało (zmiana sprzed usunięcia, niezsynchronizowana) — jedyna kopia,
  # więc wraca do repozytorium zamiast trafić do usunięcia u źródła (przegląd 22.10).
  commit_copy "z odtworzona"
  rm -f "$RULES/z.md"
  commit_copy "z usunięta drugi raz"
  echo "zasada z, zmiana nigdy niezatwierdzona" >"$ASSISTANT/z.md"
  touch -d '2026-01-01 00:00' "$ASSISTANT/z.md"
  sync
  check "nigdy niezatwierdzona treść usuniętej zasady: wraca, nie do usunięcia" eval '! out_has "ZASADY USUNIĘTE W REPOZYTORIUM" && same "$ASSISTANT/z.md" "$RULES/z.md"'
fi
# Klucz w kopii, gdy źródło wygląda na nowe — i tak zastąpiony czystą wersją.
setup
rules "$RULES" a b c d
printf 'klucz: %s\n' "$KEY" >>"$RULES/a.md"
rules "$ASSISTANT" a
sync
check "klucz w kopii przy nowym źródle: zastąpiony mimo wstrzymania" same "$ASSISTANT/a.md" "$RULES/a.md"
check "klucz w kopii przy nowym źródle: kod 3, bez klucza" eval 'rc_is 3 && no_key_in_copy'

sync --bez-sensu
check "nieznany argument: kod 2" rc_is 2
sync --help
check "--help: kod 0 i opis --prune" eval 'rc_is 0 && out_has "--prune"'

finish "memory-sync.sh"
