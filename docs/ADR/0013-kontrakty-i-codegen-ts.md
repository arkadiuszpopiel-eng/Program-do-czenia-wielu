# ADR 0013 — Kontrakty i generowanie typów: Rust → TS (`tauri-specta`/`ts-rs`), JSON Schema zdarzeń, WIT

| Pole | Wartość |
|---|---|
| Status | Zaakceptowany |
| Data | 2026-09-30 |
| Powiązane sekcje planu | §1.2 (Kontrakty), §3.2, §3.6 (`packages/schemas`), §4.3, §4.4, §13, §15 |

## Kontekst

Warstwy: Rust (jądro, moduły), TS/Svelte (UI), Wasm (wtyczki), pliki konfiguracyjne TOML i zdarzenia NDJSON. Kod piszą dwa różne modele AI w osobnych sesjach — dryf typów między warstwami jest najczęstszym źródłem błędów, których kompilator jednej warstwy nie widzi. `tauri-specta` jest w RC, `ts-rs` stabilny; wiedza modeli kończy się w IV–VI 2026.

## Decyzja

| Źródło prawdy | Generowany artefakt | Narzędzie | Kontrola |
|---|---|---|---|
| traity i typy Rust w `<m>-contract` | typy TS + bindingi komend IPC | `tauri-specta` (RC, wersja przypięta) lub `ts-rs` (wersja przypięta) — wybór ostateczny w F0 pkt 2 | `git diff --exit-code` na plikach generowanych w CI |
| typy zdarzeń Rust | JSON Schema zdarzeń w `packages/schemas/` | `schemars` (lub równoważne; wersja przypięta) | `git diff --exit-code`; testy walidacji przykładów |
| schematy konfiguracji modułów | JSON Schema w `packages/schemas/` (referencjonowane z `module.toml`) | jw. | walidacja w `core-config`, przeładowanie na żywo |
| WIT w `plugins/` | bindingi host/guest | wit-bindgen | `git diff --exit-code` |

Zasady:

- Ręczne edytowanie plików generowanych jest zabronione (hook + CI).
- Schematy zdarzeń wersjonowane; upcastery + testy migracji (§13); paczki `.alfa` niosą wersję schematu.
- Rust jest źródłem prawdy; UI nie definiuje własnych typów domenowych.
- `dependency-cruiser` dla TS pilnuje, by UI importowało typy tylko z pakietu generowanego.
- Wersje narzędzi przypięte i opisane w `docs/vendor/`.

## Alternatywy (odrzucone)

| Alternatywa | Dlaczego nie |
|---|---|
| Ręcznie pisane typy TS | dryf między sesjami AI; brak sygnału kompilatora po zmianie kontraktu |
| Schemat (OpenAPI/JSON Schema) jako źródło prawdy, generowanie Rust i TS | gorsza ergonomia w Rust (traity, enumy z danymi); Rust ma bogatszy system typów |
| Protobuf/gRPC | zbędna warstwa binarna dla IPC lokalnego; JSON w Tauri jest wystarczający, a schematy JSON są czytelne dla modeli |
| Tylko `tauri-specta` (bez planu B) | RC — jeśli okaże się niestabilny lub nieznany modelom, `ts-rs` jest zapasem |

## Konsekwencje

- Zmiana kontraktu = jeden PR ze zmianą Rust + regeneracja; recenzent widzi wpływ na UI w diffie.
- Kolejność pracy nad modułem (SPEC → kontrakt → fake → testy) opiera się na tym, że kontrakt kompiluje się osobno.
- Koszt: dodatkowy krok builda i narzędzia w toolchainie; łagodzony hookami lokalnymi.
- Ryzyko: `tauri-specta` RC — decyzja tauri-specta vs ts-rs w F0 po próbie na pierwszym module-przykładzie.

## Jak cofnąć

- Przełączenie `tauri-specta` ↔ `ts-rs` dotyczy tylko generatora; kontrakty Rust bez zmian.
- Rezygnacja z generowania oznacza powrót do ręcznych typów i utratę checku w CI — nie przewidujemy.
