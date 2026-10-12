---
name: alfa-budowanie-aplikacji
description: Aplikację Alfa budować i uruchamiać tylko co pięć dużych ulepszeń albo na polecenie właściciela — nie po każdej wersji
metadata:
  node_type: memory
  type: feedback
  originSessionId: 526ab23b-611a-489a-a8a3-7c1153e69825
  modified: 2026-10-11T19:34:25.431Z
---

2026-10-11 właściciel (Spopielony): „nie buduj za każdym razem aplikacji, jedynie co pięć dużych ulepszeń lub na
moje polecenie, bo tak tracisz czas”. Zastępuje wcześniejszą zasadę „po zakończeniu etapu uruchom nową wersję”.

**Why:** pełna budowa powłoki trwa 5–15 minut i zjada czas pracy, a on nie ogląda każdej wersji.

**How to apply:** po zmianie nadal podnosić wersję, pisać CHANGELOG i puszczać testy (Rust, vitest, E2E — te są
szybkie i celowane), ale `cargo build` powłoki, testy `e2e-app` i uruchomienie programu robić dopiero po pięciu
dużych ulepszeniach albo gdy poprosi. W raporcie pisać wprost, które wersje nie były zbudowane ani sprawdzone
w prawdziwej aplikacji. Licznik prowadzić w `memory/status.md`. Patrz też [[alfa-weryfikacja-ui]].
