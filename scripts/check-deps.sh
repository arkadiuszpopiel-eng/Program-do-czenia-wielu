#!/usr/bin/env bash
# Sprawdza graf zależności workspace (docs/PLAN.md §3.2, §4.3; crates/README.md):
#  - żaden crate workspace nie zależy od cudzego `*-impl` (w żadnym rodzaju zależności),
#  - `*-fake` innego modułu wolno używać tylko w `dev-dependencies`,
#  - zależność między modułami idzie wyłącznie przez `*-contract`,
#  - wyjątek: `lib-*` = wspólna biblioteka narzędziowa bez logiki modułu (np. szyfrowana baza);
#    wolno od niej zależeć, a sama może zależeć tylko od `lib-*` i `*-contract`.
# Użycie: scripts/check-deps.sh [--self-test]   (wymaga cargo + jq)
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

# Nazwa modułu = nazwa crate'a bez sufiksu -contract/-impl/-fake.
module_of() {
  sed -E 's/-(contract|impl|fake)$//' <<<"$1"
}

# check_edges <plik z liniami "pakiet zależność rodzaj">  → wypisuje naruszenia, zwraca 1 gdy są
check_edges() {
  local violations=0
  while read -r pkg dep kind; do
    [[ -z "$pkg" ]] && continue
    local pkg_mod dep_mod
    pkg_mod="$(module_of "$pkg")"
    dep_mod="$(module_of "$dep")"
    [[ "$pkg_mod" == "$dep_mod" ]] && continue   # ta sama trójka: impl→contract, fake→contract OK
    if [[ "$pkg" == lib-* && "$dep" != lib-* && "$dep" != *-contract ]]; then
      echo "NARUSZENIE: biblioteka $pkg może zależeć tylko od lib-* i *-contract: $dep ($kind)"; violations=1
      continue
    fi
    case "$dep" in
      *-impl)
        echo "NARUSZENIE: $pkg zależy od cudzego -impl: $dep ($kind)"; violations=1 ;;
      *-fake)
        if [[ "$kind" != "dev" ]]; then
          echo "NARUSZENIE: $pkg zależy od cudzego -fake poza dev-dependencies: $dep ($kind)"; violations=1
        fi ;;
      *-contract|lib-*) ;;
      *)
        echo "NARUSZENIE: $pkg zależy od crate'a workspace bez sufiksu -contract/-impl/-fake: $dep ($kind)"; violations=1 ;;
    esac
  done <"$1"
  return $violations
}

self_test() {
  local tmp; tmp="$(mktemp)"
  cat >"$tmp" <<'CASES'
a-impl a-contract normal
a-fake a-contract normal
b-impl a-contract normal
b-impl a-fake dev
b-impl a-impl normal
b-fake a-fake normal
c-impl d-utils normal
c-impl lib-sqlstore normal
lib-sqlstore a-contract normal
lib-sqlstore a-fake dev
CASES
  local out; out="$(check_edges "$tmp" || true)"
  rm -f "$tmp"
  local expected=4
  local got; got="$(grep -c NARUSZENIE <<<"$out" || true)"
  if [[ "$got" -ne "$expected" ]]; then
    echo "self-test: oczekiwano $expected naruszeń, wykryto $got"; echo "$out"; exit 1
  fi
  echo "self-test OK ($got/$expected naruszeń wykrytych)"
}

if [[ "${1:-}" == "--self-test" ]]; then
  self_test
  exit 0
fi

command -v jq >/dev/null || { echo "brak jq"; exit 2; }

edges="$(mktemp)"
trap 'rm -f "$edges"' EXIT

# Krawędzie: tylko między członkami workspace; rodzaj: normal / build / dev.
cargo metadata --format-version 1 --no-deps --manifest-path "$ROOT/Cargo.toml" \
  | jq -r '
      (.packages | map(.name)) as $members
      | .packages[]
      | .name as $pkg
      | .dependencies[]
      | select(.name as $n | $members | index($n))
      | "\($pkg) \(.name) \(.kind // "normal")"
    ' >"$edges"

if check_edges "$edges"; then
  echo "check-deps OK: $(wc -l <"$edges") krawędzi między crate'ami workspace, zero naruszeń"
else
  echo "check-deps: wykryto naruszenia zasady trójki crate'ów (crates/README.md)"
  exit 1
fi
