#!/usr/bin/env bash
# Sprzątanie starych kompilacji (polecenie właściciela 5.11: „koniec z zapychaniem dysku, stare
# kompilacje mają być kasowane”). Uruchamiane samo na końcu `verify.mjs` i `backup.sh`; można też ręcznie.
#
#   scripts/sprzatanie.sh           kasuje to, co poniżej; `target/debug` tylko ponad limit
#   scripts/sprzatanie.sh --wszystko  także `target/debug` niezależnie od rozmiaru
#
# Kasowane zawsze (wszystko da się odbudować):
#   - apps/desktop/src-tauri/target — osobna kopia kompilacji powłoki (powłoka buduje się do target/shell);
#   - target/trial-profile — profil starej instancji testowej;
#   - target/verify/<wersja> starszych wersji niż bieżąca (raporty bieżącej zostają).
# `target/debug` (testy i linter całego projektu) — `cargo clean --profile dev`, gdy przekracza
# ALFA_TARGET_LIMIT_GB (domyślnie 50). Zostają: target/owner (kopia właściciela) i target/shell
# (instancja testowa), bo bez nich nie działa skrót „Alfa” ani test prawdziwej aplikacji.

set -u
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT" || exit 1
LIMIT="${ALFA_TARGET_LIMIT_GB:-50}"

gb() { du -s "$1" 2>/dev/null | awk '{ printf "%d", $1 / 1048576 }'; }

before="$(gb target)"
for P in apps/desktop/src-tauri/target target/trial-profile; do
  if [ -e "$P" ]; then
    rm -rf "${P:?}" && echo "usunięto: $P"
  fi
done

current="$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)"
if [ -n "$current" ] && [ -d target/verify ]; then
  for P in target/verify/[0-9]*.[0-9]*.[0-9]*; do
    [ -d "$P" ] || continue
    [ "$(basename "$P")" = "$current" ] && continue
    rm -rf "${P:?}" && echo "usunięto: $P"
  done
fi

debug="$(gb target/debug)"
if [ "${1:-}" = "--wszystko" ] || [ "${debug:-0}" -gt "$LIMIT" ]; then
  echo "target/debug: ${debug:-0} GB (limit $LIMIT GB) — cargo clean --profile dev"
  if [ -f scripts/portable-env.sh ]; then
    # shellcheck disable=SC1091
    source scripts/portable-env.sh >/dev/null 2>&1
  fi
  cargo clean --profile dev >/dev/null 2>&1 || echo "cargo clean: błąd (budowa w toku?) — spróbuj ponownie"
fi

echo "sprzątanie: target ${before:-0} GB → $(gb target) GB"
