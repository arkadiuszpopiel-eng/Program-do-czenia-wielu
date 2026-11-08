#!/usr/bin/env bash
# Wspólne pomocnicze funkcje testów skryptów (bez bats): `source "$(dirname "$0")/lib.sh"`.
# Każdy test liczy zaliczone i niezaliczone sprawdzenia; `finish` kończy z kodem 1 przy porażce.

PASSED=0
FAILED=0
OUT="" # wyjście ostatniego `run`
RC=0   # kod wyjścia ostatniego `run`

ok() { PASSED=$((PASSED + 1)); echo "  ok    $*"; }
bad() {
  FAILED=$((FAILED + 1))
  echo "  BŁĄD  $*"
  [ -z "$OUT" ] || printf '%s\n' "$OUT" | sed 's/^/        | /'
}

# check <opis> <polecenie…> — zalicza, gdy polecenie kończy się kodem 0.
check() {
  local desc="$1"
  shift
  if "$@"; then ok "$desc"; else bad "$desc"; fi
}

# run <polecenie…> — zapisuje wyjście (stdout + stderr) w OUT i kod w RC.
run() {
  OUT="$("$@" 2>&1)"
  RC=$?
}

rc_is() { [ "$RC" -eq "$1" ]; }
out_has() { [[ "$OUT" == *"$1"* ]]; }

finish() {
  echo "$1: zaliczone $PASSED, niezaliczone $FAILED"
  [ "$FAILED" -eq 0 ]
}
