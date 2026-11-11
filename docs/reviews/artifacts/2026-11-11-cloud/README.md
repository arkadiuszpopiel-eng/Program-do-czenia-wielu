# Surowe dowody Cloud — 11.11.2026

Źródło aplikacji: baza`bfd69b974f4cab75d25f3328abe1996b10def1dc`, nowy kod UI
`2f3cf3e39a707ecb7c7a2ecee14772e37448fc88`. [Manifest SHA256 wszystkich logów i skryptów](manifest.json).
**Oryginalne bajty logów** są w [archiwum](raw-logs.tar.gz),
[manifest oryginałów](raw-logs-manifest.json). Czytelne pliki`.log` mają wyłącznie
usunięte białe znaki z końców linii i nadmiarowe puste linie na końcu pliku, aby przejść
kontrolę diff; żadnych treści, błędów lub wyników nie zmieniono. Hashe wyników w JSON
dotyczą oryginałów w archiwum, manifest ogólny — opublikowanych plików.
Nie ma tu tokenów CLI, poświadczeń właściciela ani nagłówków kont API. Lokalna autoryzacja
serwerów jest efemeryczna i zredagowana. Nie są to testy GPU, mikrofonu lub fizycznego telefonu.

## UI — polecenia i wyniki

W worktreeUI, `PATH=/workspace/tools/pnpm/node_modules/.bin:$PATH`:

```sh
pnpm --filter @alfa/desktop-ui build
pnpm --filter @alfa/desktop-ui check
pnpm --filter @alfa/desktop-ui test
pnpm lint
node scripts/cockpit-map.mjs --check
cd apps/desktop/ui
PW_WORKERS=1 pnpm exec playwright test e2e/cockpit-systems.spec.ts e2e/cockpit.spec.ts e2e/remote-preview.spec.ts --max-failures=1
```

- [Końcowe41/41, kod0](jarvis-e2e-final.log); [Vitest573/573](jarvis-vitest.log),
  [typy0/0](jarvis-check-final.log), [build0](jarvis-build-final.log).
- [Końcowe lint/format/diff: komendy, kody i hashe](jarvis-final-checks.json),
  [lint](jarvis-lint-final.log), [format](jarvis-format-final-v3.log).
- [RED: brak nowego widoku](jarvis-systems-red.log),
  [RED: nagłówek](jarvis-touch-red.log), [RED: przycisk32px](phone-touch-red.log).
- [Pierwsza seria2workery](jarvis-e2e-v1.log):2failed/2interrupted/37didnotrun,
  [własnySTOP](jarvis-e2e-v1-stop.json), kod130. Nie są zaliczeniami.
  [Ten sam kod1worker11/11](jarvis-e2e-v2-isolated.log),
  [stare30/30](jarvis-e2e-existing.log); nie sumować jako unikalnych testów.
- [Nieprawidłowy argument formatera](jarvis-format.log), kod2; poprawiona komenda powyżej0.

Zrzuty po zmianie: [katalog](../2026-11-11-atlas/),
[protokół przechwycenia](../2026-11-11-atlas/capture.json),
[skrypt](capture_jarvis.mjs). Vite tylko127.0.0.1:4173, atrapa IPC.
`pageerror=[]`, zewnętrzne żądania=[] w każdym z trzech widoków. Obrazy obejrzano,
nie tylko zapisano. Kryterium axe0critical/serious zachowane, limit testu60s bez zmian.

## Regresje zwykłego Core

[Komenda i kod0](core-jarvis-regression.json), [19/0/0 w8integracjach](core-jarvis-regression.log).
Jarvis5,własna persona2,głos1,autonomia1+1+4+3,routing2. Porty hosta są atrapami,
to nie rozmowy z rzeczywistym modelem lub mikrofonem. Źródła Rustbfd69b97 są identyczne
w kodzieUI2f3cf3e3. Nie dodajemy tych19 do innych starszych wyników.

## #58 — hipoteza i niepowodzenia

[Raport](../../2026-11-11-ledger-diagnosis.md),
[łatka niezrealizowanego prototypu testu](../../patches/58-host-prototype-tests.patch).
Nie jest częścią testów uruchamianych automatycznie lub implementacją produktu.

```sh
cargo +1.94.1 test --locked -p app-core --test execution_ledger_context host_ledger_read_reaches_the_actual_provider_without_creating_business_actions -j2 -- --exact
```

RUSTUP_HOME=/workspace/tools/rustup,CARGO_HOME=/workspace/tools/cargo,
CARGO_TARGET_DIR=/workspace/target-repair,profiledev/testdebug0,incremental0;
cmake3.31.10 w `/workspace/tools/cmake/cmake/data/bin`.
[Właściwy wynik](58-core-carrier-red-v2.json), [log](58-core-carrier-red-v2.log):
0passed/1failed/1filtered,kod101, missing proponowany hostledger.
[Pierwsze błędne przygotowanie](58-core-carrier-red.json),
[log braku celu](58-core-carrier-red.log) — nie behavioralRED.
Po archiwizacji usunięto tylko własne2nieśledzone pliki prototypu;
[hash źródeł](prototype-cleanup.json). Kod app-chat nie został zmieniony.

## Model i kontroler

Piny modelu/serwera i ich SHA są w [raporcie](../../2026-11-11-ledger-diagnosis.md),
[manifest modelu](main-model-entry.json), [weryfikacja serwera](server-verified.json),
[pełny transfer i hash](main-model-range-download.json).
[Pierwszy świadomie przerwany transfer](main-model-first-attempt.json) zachowany.

```sh
python3 /workspace/scratch/repair-20261111/probe_host_ledger.py current_text_only
python3 /workspace/scratch/repair-20261111/probe_host_ledger.py host_lookup
python3 /workspace/scratch/repair-20261111/diagnose_minimal.py diag_minimal
```

[Kontroler porównania](probe_host_ledger.py), [diagnoza minimalna](diagnose_minimal.py).
Wejściowe3przypadki to pierwsze3pozycje zachowanego V1:
`docs/reviews/artifacts/history-receipts-v1-candidate-responses.json`.
Odrzucona notaV1 jest usuwana. Laboratoryjny host_lookup syntetyzuje nośnik danych,
nie zlecone przez model wywołanie ani biznesowe działanie agentki; nie został wdrożony.

- [Baza: status, żądanie i log](raw-current_text_only/status.json):1timeout/2unrun,0tekstów.
- [Hostlookup](raw-host_lookup/status.json):1timeout/2unrun,0tekstów.
- [Minimalny serwer](raw-diag_minimal/status.json):GOTOWA,5tokenów,first6489ms,total11728ms;
  [surowe czasy serwera](raw-diag_minimal/server.log).

Kod0kontrolera oznacza zebranie danych, nie zaliczenie jakości. Źródłem wyniku jest
`results[].error`, zakończenie treści i ocena semantyki; dla dwóch ekranów
**NOT_EVALUATED**. Pingi i metadane nie aktualizują zegara generacji.
Nie ponawiano przypadków dla uzyskania zaliczenia i nie uruchomiono dalszych30po timeoutach.
