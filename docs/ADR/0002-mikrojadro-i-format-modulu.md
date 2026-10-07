# ADR 0002 — Mikrojądro i format modułu (trójka crate'ów, manifest)

| Pole | Wartość |
|---|---|
| Status | Zaakceptowany |
| Data | 2026-09-30 |
| Powiązane sekcje planu | §2 (zasady 1–3), §3.1–3.3, §3.6, §4.3, §4.5, §4.5a |

## Kontekst

Wymagania „lekki i bardzo modularny" oraz „budowany w 100% przez AI" spotykają się w jednym miejscu: moduł musi mieścić się w oknie kontekstu modelu, mieć jeden kontrakt, dać się testować bez sprzętu i nie kosztować zasobów, gdy nie jest używany. Kontekst modeli nie jest gwarantowany (zgłaszano przycięcia w Codex CLI), więc granice modułów muszą być twarde.

## Decyzja

1. **Mikrojądro „Alfa Core"** (≈ kilka MB): `core-bus`, `core-registry`, `core-config`, `core-log`, klient Brokera, `updater`. Bez logiki domenowej.
2. **Moduł = trójka crate'ów:** `<m>-contract` (trait + typy + zdarzenia + JSON Schema), `<m>-impl`, `<m>-fake`. Inne moduły zależą **tylko od `-contract`**; egzekwowane sprawdzaniem grafu zależności w CI.
3. **Manifest `module.toml`:** `id`, `version`, `kind` (service/tool/provider/voice-engine/agent-pack/ui-panel), kontrakty dostarczane/wymagane, żądane zdolności, budżet zasobów (RAM/CPU), cykl życia (`lazy|on-demand|always`), izolacja (`inproc|process|wasm`), schemat konfiguracji, wkład do UI (panel, strona ustawień), health-check. Schemat manifestu powstaje w F0 (pkt 2 §4.5a).
4. **Izolacja wg potrzeby:** in-proc dla audio i schedulera (wątek RT, bez Wasm/IPC w callbacku audio), osobny proces dla ciężkich/awaryjnych (JSON-RPC po stdio/named pipe; AppContainer + Job Object dla niezaufanych), Wasm dla kodu generowanego przez AI.
5. **Kompozycja:** feature flags wybierają skład builda (minimalny = sam czat); w runtime hot-włączanie dla proces/Wasm, aktywacja flagą dla in-proc.
6. **Heurystyki rozmiaru:** plik ≤ ~300–400 linii, crate ≤ ~5–8 tys. linii (clippy `too_many_lines`, skrypty CI). Liczby są heurystyką, nie wynikiem badań.
7. **Kolejność pracy:** SPEC → kontrakt → fake → testy → implementacja → przegląd drugiego modelu → CI → merge. Pierwszy moduł-przykład z trójką crate'ów i testem kontraktowym powstaje w F0 jako wzorzec.

## Alternatywy (odrzucone)

| Alternatywa | Dlaczego nie |
|---|---|
| Monolit z modułami jako katalogami | brak twardych granic; moduł nie mieści się w kontekście AI; nie da się zwolnić nieużywanego kodu |
| Wtyczki DLL przez `abi_stable` | ABI niestabilne między wersjami kompilatora, brak sandboxu, trudne dla kodu generowanego przez AI — odrzucone w §3.2 |
| Dwójka crate'ów (contract + impl) bez `-fake` | testy bez sprzętu i bez sieci wymagają atrapy; bez niej AI nie może weryfikować samodzielnie (§4.5) |
| Manifest w kodzie (makra) zamiast TOML | rejestr musi czytać manifest bez ładowania modułu (lazy); TOML jest edytowalny i walidowalny JSON Schema |

## Konsekwencje

- Każdy moduł ma `docs/modules/<m>/SPEC.md` (1 strona): cel, kontrakt, niezmienniki, testy akceptacyjne, budżety.
- Testy kontraktowe uruchamiane przeciwko `-impl` i `-fake`; rozjazd = błąd.
- Budżet z manifestu monitorowany w runtime; przekroczenie → ostrzeżenie/zwolnienie.
- Więcej crate'ów w workspace (≈ 3 × liczba modułów) — koszt kompilacji; łagodzony feature flagami i cache CI.
- Jedna sesja AI = jeden moduł = jeden git worktree.

## Jak cofnąć

- Scalenie `-impl` z `-contract` w wybranych modułach jest mechaniczne, ale łamie zasadę „zależność tylko od kontraktu"; wymagałoby zdjęcia checku grafu w CI.
- Rezygnacja z `-fake` możliwa per moduł tylko tam, gdzie istnieje inny deterministyczny symulator.
- Decyzja odwracalna do końca F0.
