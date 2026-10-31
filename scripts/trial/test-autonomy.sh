#!/usr/bin/env bash
# Poziom autonomii instancji testowej (docs/plan/45 §6.2): `ALFA_TEST_AUTONOMY=L4` zakłada w profilu
# TYMCZASOWYM plik zapamiętanego poziomu Brokera (`broker-dev/autonomy-standing.json`) — tak, jakby
# właściciel raz zatwierdził „Pełną automatykę” w tym profilu. Broker w procesie czyta plik przy starcie
# (`app-broker/src/inproc.rs`, scalone po przeglądzie właściciela 31.10). Profil właściciela nigdy nie
# jest dotykany: funkcja odmawia katalogu spoza `target/`.
#
#   source scripts/trial/test-autonomy.sh; seed_test_autonomy "<profil>/Local"
seed_test_autonomy() {
  local local_dir="$1" level="${ALFA_TEST_AUTONOMY:-}"
  [ -n "$level" ] || return 0
  case "$level" in L0 | L1 | L2 | L3 | L4) ;; *)
    echo "ALFA_TEST_AUTONOMY: nieznany poziom „$level” (L0–L4)" >&2
    return 1
    ;;
  esac
  case "$local_dir" in */target/*) ;; *)
    echo "ALFA_TEST_AUTONOMY: odmowa — $local_dir nie jest profilem tymczasowym w target/" >&2
    return 1
    ;;
  esac
  mkdir -p "$local_dir/Alfa/broker-dev"
  printf '{"level":"%s"}' "$level" >"$local_dir/Alfa/broker-dev/autonomy-standing.json"
  echo "instancja testowa: poziom $level (profil tymczasowy)"
}
