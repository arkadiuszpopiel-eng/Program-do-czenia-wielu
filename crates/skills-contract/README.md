# skills-contract

Kontrakt modułu `skills` (docs/modules/skills/SPEC.md, PLAN §9.5, §12.1 R1).

- `Skill` — nazwany, wersjonowany (semver) przepis: opis, słowa kluczowe, wymagane narzędzia i rodziny
  zdolności (muszą równać się zdolnościom z manifestów; `system.admin`/`secrets.read` zakazane), parametry
  (podzbiór JSON Schema: obiekt zamknięty, typy proste, `enum`, `required`, `default`, limity), szablon
  `{{parametr}}`, kroki, przykłady, testy akceptacyjne (≥ 1, deterministyczne).
- `SkillLibrary` — rdzeń wspólny dla `-impl`/`-fake`: propozycja (walidacja + testy + skaner treści) →
  kwarantanna (źródło niezaufane albo podejrzana treść spoza właściciela) → zatwierdzenie właściciela
  (`OwnerApproval` z hashem przejrzanej treści; kwarantanna tylko z okna) → instalacja; aktualizacja =
  wyższa wersja, stara „zastąpiona”; eksport/import paczki `alfa.skills.v1` z SHA-256 (import = propozycje).
- `runnable_by` / `prepare_run` — uprawnienia umiejętności ≤ roli wywołującej; przebieg `agent-runtime`
  z kopertą `RunGrant` = wymagania ∩ koperta wywołującej, budżet ∩, pochodzenie i taint wywołującej.
- `search` (rdzenie słów po `fold`, wagi pól, próg) + `SkillRanker`/`rerank` (model tylko przestawia).
- `draft_from_memory` — szkic z warstwy proceduralnej pamięci (proweniencja → zaufanie).
- Trait `Skills`, zdarzenia `skills.*` (bez treści), `samples` i `contract_tests` (feature).
