# ADR 0016 — Jeden wykonawca (Claude Code); recenzja w świeżym kontekście zamiast drugiego modelu

| Pole | Wartość |
|---|---|
| Status | Zaakceptowany (decyzja właściciela z 2026-10-07) |
| Data | 2026-10-08 |
| Powiązane sekcje planu | §4 (budowa przez AI), §4.6 (bramki ludzkie), §18; `docs/AI_WORKFLOW.md` §2, `AGENTS.md` |
| Zastępuje | zapis o przeglądzie krzyżowym Opus ↔ GPT-6 Sol w PLAN §4 i AI_WORKFLOW §2.1 |

## Kontekst

Plan zakładał dwóch wykonawców — Claude Code (Opus 5.5) i Codex CLI (GPT-6 Sol) — oraz zasadę „kto napisał, ten nie recenzuje", w której recenzję robi model z innej rodziny. W praktyce cały kod (128 commitów do 2026-10-07) napisały sesje Claude, a Codex nie brał udziału ani jako autor, ani jako recenzent. Właściciel potwierdził, że Codex nie ma recenzować — zapis w dokumentach był błędem. Dokumenty sterujące muszą opisywać proces, który faktycznie działa.

## Decyzja

| Element | Wybór |
|---|---|
| Wykonawca | **Claude Code** (bieżący model Opus) — cały kod, testy, dokumentacja, CI |
| Codex CLI / GPT-6 Sol w budowie | **nie uczestniczy** — ani jako autor, ani jako recenzent |
| „Model-recenzent", „recenzent", „drugi model" w dokumentach | **osobna sesja Claude w świeżym kontekście**, która nie pisała recenzowanego kodu; wejście: diff, SPEC, kontrakt, wyniki CI — bez historii sesji autorskiej |
| Zakres recenzji | bez zmian: poprawność, bezpieczeństwo, zgodność ze SPEC i kontraktem, spełnienie DoD |
| Zestawy akceptacyjne fal | tworzy sesja-recenzentka (nie sesja autorska), akceptuje człowiek, potem zamrożenie hashem w `evals/` |
| Jądro, Broker, polityki bezpieczeństwa | bez zmian: **obowiązkowy przegląd i akceptacja człowieka** (bramka #7) |
| Codex i Claude Code jako funkcja programu | bez zmian — mosty CLI (`agent-backends`, F4) to cecha Alfy, niezależna od tego, kto ją buduje |

Ponieważ autor i recenzent są z tej samej rodziny modeli, ryzyko wspólnych ślepych punktów rośnie. Wyrównują je trzy reguły, które od teraz są częścią Definition of Done:

1. **Kryterium akceptacji jest spełnione dopiero po przebiegu na prawdziwej maszynie.** `docs/STATUS.md` rozróżnia „✅ CI" (atrapy) i „✅ sprzęt".
2. **Każdy błąd znaleziony na sprzęcie dostaje test, który by go złapał** — w pierwszej kolejności test prawdziwej aplikacji (`apps/desktop/ui/e2e-app`), nie test na atrapie.
3. **Przegląd człowieka nad Jądrem jest niezbywalny** — sesja AI przygotowuje pakiet do przeglądu, nie zatwierdza.

## Alternatywy (odrzucone)

| Alternatywa | Dlaczego nie |
|---|---|
| Utrzymać zapis o recenzji GPT-6 Sol | opisuje proces, który nie istnieje; fałszywe poczucie niezależnej kontroli |
| Zrezygnować z recenzji w ogóle | świeży kontekst nadal łapie błędy wynikające z założeń sesji autorskiej; trzy przeglądy bezpieczeństwa w `docs/reviews/` znalazły ustalenia o ocenie do 8,1 |
| Recenzja przez człowieka dla każdego modułu | ok. 100 modułów — nierealne dla jednej osoby; człowiek zostaje przy Jądrze i decyzjach |

## Konsekwencje

- `AGENTS.md`, `docs/AI_WORKFLOW.md` §2, `README.md` i `docs/modules/README.md` są poprawione zgodnie z tą decyzją. W `docs/PLAN.md` §4 jest odsyłacz do tego ADR; tabele „pierwszych dwóch tygodni" zostają jako zapis historyczny.
- Zapisy „tworzy model-recenzent" w `docs/ACCEPTANCE.md` i `docs/STATUS.md` pozostają w mocy w znaczeniu z tabeli wyżej.
- Uwaga o małym oknie kontekstu Codexa nie jest już powodem limitów rozmiaru; limity (plik ≤ 400 linii, crate ≤ 8 000) zostają, bo służą recenzji w świeżym kontekście.
- Powrót do recenzji przez inną rodzinę modeli jest możliwy bez zmian w kodzie — wymaga nowego ADR.
