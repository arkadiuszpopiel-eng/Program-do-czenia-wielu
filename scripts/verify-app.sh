#!/usr/bin/env bash
# Budowa powłoki z cechą `e2e` i test prawdziwej aplikacji jako INSTANCJA TESTOWA (docs/plan/24):
# własny profil tymczasowy i własny port CDP, więc uruchomiona Alfa właściciela zostaje nietknięta
# (działa z osobnej kopii — `scripts/deploy-owner.sh`). Na końcu pokrycie poleceń rdzenia.
#
#   scripts/verify-app.sh [--no-build]
# Wynik: target/verify/app/{shell-build.log,smoke.log,out/,ipc-coverage.json}
set -uo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
OUT="$ROOT/target/verify/app"
PORT="${ALFA_TRIAL_PORT:-9333}"
cd "$ROOT"
source scripts/portable-env.sh >/dev/null 2>&1
mkdir -p "$OUT/out"
export CARGO_INCREMENTAL=0
if [ "${1:-}" != "--no-build" ]; then
  (cd apps/desktop/ui && pnpm build >"$OUT/ui-build.log" 2>&1) || { echo "budowa interfejsu: błąd"; exit 1; }
  (
    cd apps/desktop/src-tauri &&
      CARGO_TARGET_DIR="$(cygpath -w "$ROOT/target/shell")" CARGO_PROFILE_DEV_DEBUG=0 \
        ALFA_BUILD_DATE="$(date +%F)" ALFA_BUILD_COMMIT="$(git rev-parse --short HEAD)" \
        cargo build --locked -j 3 --features "e2e tauri/custom-protocol" >"$OUT/shell-build.log" 2>&1
  )
  code=$?
  echo "budowa: kod $code, $(tail -1 "$OUT/shell-build.log")"
  [ "$code" -eq 0 ] || exit "$code"
  ORT="${ALFA_ORT_DLL:-$LOCALAPPDATA/Alfa/sidecars/onnxruntime/onnxruntime.dll}"
  ALFA_ORT_DLL="$ORT" bash scripts/stage-sidecars.sh >"$OUT/stage.log" 2>&1
  echo "sidecary: $(tail -1 "$OUT/stage.log")"
fi
PROFILE="$OUT/profil-$(date +%H%M%S)"
mkdir -p "$PROFILE/Roaming" "$PROFILE/Local"
rm -f "$OUT/out/ipc-seen.txt"
APPDATA="$(cygpath -w "$PROFILE/Roaming")" LOCALAPPDATA="$(cygpath -w "$PROFILE/Local")" \
  powershell -NoProfile -ExecutionPolicy Bypass -File apps/desktop/ui/e2e-app/run.ps1 \
  -Exe "$(cygpath -w "$ROOT/target/shell/debug/alfa-desktop.exe")" \
  -OutDir "$(cygpath -w "$OUT/out")" -Port "$PORT" >"$OUT/smoke.log" 2>&1
smoke=$?
echo "test prawdziwej aplikacji: kod $smoke"
sed 's/\x1b\[[0-9;]*m//g' "$OUT/smoke.log" | grep -E "^\s+[0-9]+ (passed|failed|flaky)|^\s+x\s" | cut -c1-170
# Bramka pokrycia łącza (audyt A-09): spadek poniżej progu = błąd biegu, nie tylko liczba w raporcie.
node scripts/ipc-coverage.mjs "$OUT/out" --json "$OUT/ipc-coverage.json" --min "${ALFA_IPC_MIN:-70}" >"$OUT/ipc-coverage.log" 2>&1
coverage=$?
head -3 "$OUT/ipc-coverage.log"
[ "$smoke" -eq 0 ] || exit "$smoke"
exit "$coverage"
