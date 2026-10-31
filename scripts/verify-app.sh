#!/usr/bin/env bash
# Budowa powłoki z cechą `e2e` i test prawdziwej aplikacji jako INSTANCJA TESTOWA (docs/plan/24):
# własny profil tymczasowy i własny port CDP, więc uruchomiona Alfa właściciela zostaje nietknięta
# (działa z osobnej kopii — `scripts/deploy-owner.sh`). Na końcu pokrycie poleceń rdzenia.
#
#   scripts/verify-app.sh [--no-build]
#   ALFA_APP_SPEC=z-katalog scripts/verify-app.sh --no-build   # tylko wybrane pliki testów
# Wynik: target/verify/app/{shell-build.log,smoke.log,out/,ipc-coverage.json}
set -uo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
# Wybrane pliki (ALFA_APP_SPEC): osobny katalog wyniku — ślad pełnego biegu i bramka pokrycia zostają.
OUT="$ROOT/target/verify/app${ALFA_APP_SPEC:+-wybrane}"
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
        ALFA_BUILD_DATE="$(date +%F)" ALFA_BUILD_COMMIT="$(git -c safe.directory='*' -C "$ROOT" rev-parse --short HEAD)" \
        cargo build --locked -j 3 --features "e2e tauri/custom-protocol" >"$OUT/shell-build.log" 2>&1
  )
  code=$?
  echo "budowa: kod $code, $(tail -1 "$OUT/shell-build.log")"
  [ "$code" -eq 0 ] || exit "$code"
  # ONNX Runtime dla procesów pobocznych mowy: podany w ALFA_ORT_DLL albo pierwszy z sumą przypiętą
  # w katalogu modeli („onnxruntime-cpu”, app-models/src/voice_runtime.rs) — z „Modeli lokalnych
  # i silników” albo z kopii właściciela. 21.10 na PC bez pobranego ORT sidecary zostały niepełne.
  ORT_SUM="$(grep -A3 'member: format!("{folder}/lib/{ORT_FILE}")' crates/app-models/src/voice_runtime.rs |
    grep -o '[0-9a-f]\{64\}' | head -1)"
  ORT="${ALFA_ORT_DLL:-}"
  if [ -n "$ORT" ] && [ -f "$ORT" ] && [ "$(sha256sum "$ORT" | cut -d' ' -f1)" != "$ORT_SUM" ]; then
    echo "uwaga: ALFA_ORT_DLL ma inną sumę niż przypięta w katalogu modeli ($ORT)"
  fi
  if [ -z "$ORT" ]; then
    for c in "$LOCALAPPDATA/Alfa/sidecars/onnxruntime/onnxruntime.dll" \
      "$ROOT/../Alfa-uruchomiona/sidecars/supertonic/onnxruntime.dll"; do
      if [ -n "$ORT_SUM" ] && [ -f "$c" ] && [ "$(sha256sum "$c" | cut -d' ' -f1)" = "$ORT_SUM" ]; then
        ORT="$c"
        break
      fi
    done
  fi
  if [ -n "$ORT" ] && ALFA_ORT_DLL="$ORT" bash scripts/stage-sidecars.sh >"$OUT/stage.log" 2>&1; then
    echo "sidecary: $(tail -1 "$OUT/stage.log") (ONNX Runtime: $ORT)"
  else
    # Bez kompletu z tej budowy nie zostawiamy starego: wgranie ma odmówić, a nie wgrać stare procesy.
    rm -rf "$ROOT/target/shell/debug/sidecars"
    echo "sidecary: NIEPEŁNE (${ORT:-brak onnxruntime.dll z przypiętą sumą}) — usunięte, wgranie (deploy-owner.sh) odmówi; $OUT/stage.log"
  fi
fi
PROFILE="$OUT/profil-$(date +%H%M%S)"
mkdir -p "$PROFILE/Roaming" "$PROFILE/Local"
source scripts/trial/test-autonomy.sh && seed_test_autonomy "$PROFILE/Local" || exit 2
rm -f "$OUT/out/ipc-seen.txt"
APPDATA="$(cygpath -w "$PROFILE/Roaming")" LOCALAPPDATA="$(cygpath -w "$PROFILE/Local")" \
  powershell -NoProfile -ExecutionPolicy Bypass -File apps/desktop/ui/e2e-app/run.ps1 \
  -Exe "$(cygpath -w "$ROOT/target/shell/debug/alfa-desktop.exe")" \
  -OutDir "$(cygpath -w "$OUT/out")" -Port "$PORT" ${ALFA_APP_SPEC:+-Spec "$ALFA_APP_SPEC"} >"$OUT/smoke.log" 2>&1
smoke=$?
echo "test prawdziwej aplikacji: kod $smoke"
sed 's/\x1b\[[0-9;]*m//g' "$OUT/smoke.log" | grep -E "^\s+[0-9]+ (passed|failed|flaky)|^\s+x\s" | cut -c1-170
[ -z "${ALFA_APP_SPEC:-}" ] || exit "$smoke"
# Bramka pokrycia łącza (audyt A-09): spadek poniżej progu = błąd biegu, nie tylko liczba w raporcie.
node scripts/ipc-coverage.mjs "$OUT/out" --json "$OUT/ipc-coverage.json" --min "${ALFA_IPC_MIN:-70}" >"$OUT/ipc-coverage.log" 2>&1
coverage=$?
head -3 "$OUT/ipc-coverage.log"
# Pełne uprawnienia testów AI (docs/plan/45 §6.2): ten sam program na „Pełnej automatyce” — poziom
# z profilu i przegląd każdego ustawienia na L4 (świeży profil tymczasowy dla każdego pliku).
l4=0
if [ -z "${ALFA_TEST_AUTONOMY:-}" ]; then
  for spec in z-pelny-dostep z-katalog; do
    ALFA_TEST_AUTONOMY=L4 ALFA_APP_SPEC="$spec" bash scripts/verify-app.sh --no-build >"$OUT/l4-$spec.log" 2>&1 || l4=1
    echo "Pełna automatyka (L4), $spec: $(tail -1 "$OUT/l4-$spec.log" | sed 's/^ *//')"
  done
fi
[ "$smoke" -eq 0 ] || exit "$smoke"
[ "$l4" -eq 0 ] || exit 1
exit "$coverage"
