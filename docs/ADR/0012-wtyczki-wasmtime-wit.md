# ADR 0012 — Wtyczki AI: wasmtime + własny WIT (cel `wasip2`), bez importów WASI

| Pole | Wartość |
|---|---|
| Status | Zaakceptowany |
| Data | 2026-09-30 |
| Powiązane sekcje planu | §1.2 (Wtyczki AI, Kontrakty), §3.2, §3.6, §4.3, §8.7, §12.1 (R2), §16.2 (F8), §17 |

## Kontekst

Agentki (Kreator, Ulepszacz, Diagnosta) generują kod: umiejętności, adaptery dostawców, wtyczki. Kod z modelu jest niezaufany — musi działać w sandboxie z limitami czasu i pamięci, bez dostępu do systemu plików ani sieci inaczej niż przez zdolności wydane przez Broker. Modele AI mają wiedzę do IV–VI 2026: WASI 0.3 i wasmtime 46 są dla nich świeże.

## Decyzja

| Element | Wybór |
|---|---|
| Runtime | **wasmtime**, wersja przypięta (znana modelom lub opisana w `docs/vendor/wasmtime.md`) |
| Interfejs | **własny WIT** Alfy + wit-bindgen; cel **`wasip2`** (WASI 0.3 zbyt świeże) |
| Importy WASI | **brak** — wtyczka nie ma fs, sieci, zegara ani env z WASI; wszystko przez funkcje hosta z WIT, za tokenami zdolności |
| Limity | epoch (czas), fuel (instrukcje), pamięć; przekroczenie = przerwanie wtyczki, zdarzenie Diagnostyka |
| Pierścień zmian | **R2** — wtyczki Wasm zatwierdzane 1 klikiem po testach; podpis kluczem chronionym TPM (opcjonalnie Hello) |
| Zaufanie | hash modułu i opisu narzędzi; poziomy zaufania jak dla MCP |
| Miejsce w repo | `plugins/` (źródła + WIT), `plugin-runtime` jako moduł (F8) |

Wtyczki nigdy nie działają w callbacku audio ani na wątku RT (§3.2). Ulepszacz nie może przez wtyczkę zmieniać tagów prywatności, budżetów, uprawnień, egress-allowlisty ani progów bramki ewaluacyjnej.

## Alternatywy (odrzucone)

| Alternatywa | Dlaczego nie |
|---|---|
| DLL / `abi_stable` | brak sandboxu; ABI zależne od kompilatora; kod z modelu w procesie jądra |
| Skrypty (Lua, Rhai, Python) | Lua/Rhai — słabsze limity pamięci i brak typowanego kontraktu; Python — ciężki runtime (ADR 4) |
| Osobny proces natywny per wtyczka | cięższe (proces + IPC), trudniejsze limity fuel; zostaje dla ciężkich modułów, nie dla drobnych umiejętności |
| WASI 0.3 / komponenty async | zbyt świeże dla modeli AI i toolchainu; rewizja po stabilizacji |
| Standardowe importy WASI (`wasi:filesystem`, `wasi:sockets`) | omijają Broker i tokeny zdolności; własny WIT wymusza przejście przez zdolności |

## Konsekwencje

- Definicje WIT w `plugins/` są kontraktem — zmiany wersjonowane; w CI `git diff --exit-code` na bindingach generowanych (ADR 13).
- Funkcje hosta w WIT odzwierciedlają zdolności (`fs.read(zakres)`, `net.egress(host)` itd.); wtyczka bez tokenu dostaje błąd, nie pusty wynik.
- Testy: wtyczka próbująca przekroczyć fuel/epoch/pamięć jest przerywana; wtyczka bez uprawnień nie czyta pliku — część zestawu „agentka zmienia Jądro" (F3/F8).
- Umiejętności = playbooki + narzędzia; import MCP z hashowaniem opisów.
- Koszt: wasmtime w binarium (kilka MB) — poza buildem minimalnym (feature flag), ładowany przy pierwszym użyciu.

## Jak cofnąć

- Wtyczki opisane WIT można przenieść do innego runtime komponentów (np. inny silnik Wasm) bez zmiany definicji.
- Migracja na WASI 0.3 po stabilizacji jest dodatkiem: nowy cel kompilacji, ten sam WIT Alfy.
- Rezygnacja z Wasm na rzecz procesów natywnych oznacza utratę limitów fuel i lekkości; nie przewidujemy.
