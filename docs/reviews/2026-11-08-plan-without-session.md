# Plan bez rozmowy — regresja zakresu, 8.11.2026

Karta [#25](https://github.com/arkadiuszpopiel-eng/Program-do-czenia-wielu/issues/25), dalsza praca
według [planu 54](../plan/54-jarvis-i-tryby-2026-11-08.md). Podstawa: `9b8b7416`, Linux Cloud.

## Problem i zmiana

Wybór Planu bez aktywnej rozmowy wyliczał zakres `null` i wysyłał `requestLevel('L0', null)`.
Nowa regresja przed poprawką była czerwona dokładnie na tym wywołaniu. Plan według Z.5 dotyczy
rozmowy, więc interfejs teraz prosi o jej otwarcie i nie wysyła globalnej zmiany. Alt+M pomija
Plan bez rozmowy, aby skrót mógł wrócić z Automatycznie do Ręcznie. Pozostałe tryby globalne
zachowują działanie. Komunikaty PL/EN opisują wybrany zakres zamiast zawsze mówić o wszystkich rozmowach.

## Sprawdzenie

- Testy zakresu i żądań trybów: **24/24**. Pięć nowych przypadków: Plan bez rozmowy, trzy inne
  tryby globalne bez rozmowy i cykl skrótu bez rozmowy.
- Całe Vitest: **572 zaliczone / 1 nieudany z 573**. Nieudany test to budżet wyszukiwania
  rozmytego (`fuzzy.test.ts`, około 51 ms na znak przy progu 16 ms). Równolegle trwały Rust i E2E;
  wyniku nie zaliczono ani nie zmieniono progu. Wymaga powtórzenia bez konkurujących obciążeń.
- Svelte/TypeScript: **0 błędów i 0 ostrzeżeń**. ESLint zmienionych plików: zaliczony.
- Budowa produkcyjna i budżet pakietu: zaliczone; główne okno **JS 134,6 KB gzip / 150 KB**,
  **CSS 13,9 KB / 30 KB**.

Logi: `/workspace/scratch/plan-no-session-red.log`, `plan-no-session-green.log`,
`plan-ui-unit-all.log`, `plan-ui-check-final.log`, `plan-ui-bundle.log`, `plan-no-session-eslint.log`.

## Stan szerokich prób

O 21:03 UTC stare procesy były zakończone, bez końcowego podsumowania. Pierwszy pełny log UI
zawiera 434 zaliczone przypadki; Rust kończy się na kompilacji. To próby przerwane, nie pełne zaliczenie.
Wznowiono pełne 856 E2E na źródłach `9b8b7416`. Rust uruchamiany jest jako workspace bez app-core,
a następnie wszystkie cele app-core w partiach po osiem, z `cargo clean -p app-core` między partiami:
same pliki wykonywalne testów app-core zajęły około 20 GB na dysku Cloud o pojemności 32 GB.
Logi partii i lista wszystkich celów: `/workspace/scratch/rust-batches/`.

Ta poprawka powstaje w osobnym worktree, więc nie zmienia źródeł trwających szerokich prób.
Próba rzeczywistego Bielika pozostaje po ich zakończeniu. Wersja 0.0.85, etap J i niezależny
przegląd pozostają otwarte; nie potwierdzono Windows/WebView2 ani rzeczywistego mikrofonu.
