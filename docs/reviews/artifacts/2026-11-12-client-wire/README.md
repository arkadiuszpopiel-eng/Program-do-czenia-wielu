# Pierwszy format klient–host — dowody lokalne

Pełne logi/receipty i manifest hashy, z kodem RED oraz hashem każdego końcowego źródła.

- Pierwsza kompilacja101: nie-ASCII w literale bajtowym testu;0 testów. To nie behawioralnyRED.
- Po poprawie fixture: **6passed/4failed**, kod101 (wersja, zakresID, replay/starszyID, zużyteID).
- Implementacja kontroli: **10/10**; rozszerzenie granic/właściwości: **15/15**.
- Pełne app-api + lib-client-wire: **55/0/0**, cargo test --locked -p app-api -p lib-client-wire -j2.
- Clippy --workspace --all-targets --locked -j2 -- -D warnings: **0**, bez ostrzeżeń.
- cargo-deny: **4OK**, zachowane bazowe ostrzeżenia duplikatów/nieużywanych wyjątków licencji.
- fmt/graf zależności1811krawędzi/generator398typów/docs104/lint/format: **0**.
- Pierwsze pnpm check1: w nowym worktree brak node_modules pakietów,129błędów/1ostrzeżenie.
  pnpm install --offline --frozen-lockfile0, potem bez zmiany kodu pnpm check **0błędów/0ostrzeżeń**.
- Kontrola wielkości po dodaniu plików do indeksu:3580plików/265crate'ów,0naruszeń.

Kod testowany na baziec06ced13 z pokazanym diffem; wire-final-source.json wiąże dokładne bajty
z końcowym commitem. Nie jest to wynik CI191, które dotyczy wcześniejszego372bfcd5.
Nie uruchamiano ponownie modeli, UIE2E, GPU, telefonu, sieci albo prawdziwegoAPI dla tej biblioteki.
Niezależny przegląd i noweCI integracji pozostają wymagane przed merge.
