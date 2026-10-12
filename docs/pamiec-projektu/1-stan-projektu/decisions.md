# Decision Log — Alfa AI Program
<!-- AUTO-ŁADOWANY przez SessionStart hook. -->
<!-- Dodawaj wpisy po każdej ważnej decyzji architektonicznej lub technicznej. -->
<!-- Format ADR (Architecture Decision Record) lite. -->

## Szablon wpisu

```markdown
### [YYYY-MM-DD] Tytuł decyzji
**Kontekst:** Dlaczego musieliśmy coś zdecydować?
**Opcje:** A (zalety/wady) vs B (zalety/wady)
**Decyzja:** Opcja X
**Powód:** Główna przyczyna
**Konsekwencje:** Co to zmienia/ogranicza
**Status:** Aktywna | Pod rewizją | Cofnięta
```

---

## Stack & Architektura
<!-- Decyzje fundamentalne, rzadko zmieniane -->

<!-- Przykład:
### [2026-01-15] Baza danych: PostgreSQL
**Kontekst:** Potrzebny relacyjny storage z JSONB.
**Decyzja:** PostgreSQL 16
**Powód:** JSONB + ACID + doświadczenie zespołu
**Status:** Aktywna
-->

---

### [2026-10-09] Wiele platform i dostęp zdalny (plan v6)
**Kontekst:** Właściciel chce Alfy na Windows 11 (pierwsza wersja), potem Linux, panel WWW przez Internet i Android 15+.
**Opcje:** przekaźnik z E2E / własny tunel użytkownika / oba; zgody zdalne: tylko podgląd / niskie i średnie / wszystkie;
klienci: tylko pilot / pilot + synchronizacja / pełna synchronizacja.
**Decyzja:** przekaźnik + szyfrowanie end-to-end (komputer nie otwiera portu, parowanie QR); zgody zdalne dla
wszystkich poziomów ryzyka; telefon i panel WWW jako pilot 100 % funkcji, bez komputera — rozmowa przez ten sam model
chmurowy (API), model lokalny tylko na komputerze; plan v6 obok planu v5.
**Powód:** wybór właściciela (odpowiedzi z 2026-10-09).
**Konsekwencje:** zmienia ustalenia planu v5 (tylko Windows, bez synchronizacji, sekrety tylko na komputerze, zgoda
tylko fizyczna) — projekty ADR 0017–0024 w `alfa/docs/ADR/`, do podpisu właściciela przed powstaniem kodu.
**Status:** Aktywna (ADR-y: Proponowane)

---

## Decyzje implementacyjne
<!-- Mniejsze decyzje (biblioteki, wzorce, konwencje) -->


---

## Anty-wzorce
<!-- Co świadomie odrzucaliśmy i dlaczego — żeby nie wracać do tego za miesiąc -->

## 2026-10-11 — zgoda właściciela na cztery decyzje po wersjach 0.0.19–0.0.22

Właściciel napisał: „Decyzje dla Ciebie od punktu 1 do 4 — masz zgodę na pracę”. Dotyczy:
1. Kamery sieciowe (C3.5): wolno dodać bibliotekę do dekodowania obrazu z kamer i pracować nad wyszukiwaniem
   kamer w sieci lokalnej (moja interpretacja: zgoda obejmuje pobranie zależności z crates.io i zakres „sieć lokalna”;
   nie obejmuje modeli AI ani zmian w `lib-netguard` bez przeglądu — to nadal obszar właściciela).
2. Generator typów (B.1): `schemars` + własny skrypt zostaje zamiast `tauri-specta`/`ts-rs`.
3. Etap 2 generatora: wybór sposobu (wyliczenia w Rust albo zawężanie w oknie) należy do mnie.
4. Zestaw `evals/V6/pl-core`: zaakceptowany do dalszej pracy; progów liczbowych właściciel nie podał — proponuję
   je i pokazuję do potwierdzenia.


## 2026-10-11 — głos: lokalnie, nie z chmury

Właściciel: głos przez API będzie za drogi; kazał zbadać silniki lokalne i konkurencję, potem zaakceptował
rekomendację („OK akceptuję rekomendacje twoje, wykonaj to najbardziej profesjonalnie”).
Rekomendacja: dwa poziomy głosu lokalnego — „Naturalny” (Chatterbox Multilingual V3, MIT, przez `chatterbox.cpp`)
i „Szybki” na procesorze (Supertonic 3 po próbie; do tego czasu Piper). Głos z chmury (C1.1a, ADR 0026) schodzi
na koniec kolejki jako opcja. Dokument: `alfa/docs/plan/15-glos-lokalny.md`, zadania C5.1–C5.7.
Moja interpretacja: akceptacja dotyczy kierunku i dodatku do planu; NIE jest zgodą na pobranie silników
(decyzja 1 z dokumentu 15), na znak wodny, ani wyborem źródła próbek głosów — o to pytam osobno.

## 2026-10-11 (wieczór) — Alfa programem docelowym; przeniesienie z Delty; wielu użytkowników

Właściciel po porównaniu z Deltą AI 0.8.35 (`E:\PROGRAMY W CZASIE PRAC\Delta_AI_0.8.35_KOMPLET_2026-09-18`):
- Alfa jest programem docelowym; z Delty przenosimy system rozmowy i mowy oraz WSZYSTKIE brakujące funkcje,
  skrupulatnie (etap P, dokumenty `alfa/docs/plan/16`, `17`; macierz funkcji = zadanie P0).
- Głos Edge „Zofia” tylko jako opcja wyłączona domyślnie, nigdy w sesji prywatnej.
- ADR 0025 (dane biometryczne): „możesz wykonać w pełni, masz zgodę” — przyjęty.
- Biblioteka multimediów nie musi być przenoszona; system odtwarzania i sterowania tak, rozbudowany.
- STAŁA ZGODA na pobieranie plików i modeli do 3 GB bez pytania; powyżej — pytać.
- Wielu użytkowników po koncie i logowaniu, na Windows, Linux, panelu WWW i Androidzie (etap U, ADR 0027 —
  Proponowany; moja interpretacja: zlecenie kierunku, podpis pod ADR nadal potrzebny, bo zmienia Brokera).
- Agentki mają programować jak asystent programisty: pisać, uruchamiać i budować oprogramowanie z pełnymi
  uprawnieniami w projekcie, rozmawiać ze sobą w trakcie pracy, dobierać model do trudności zadania (P12).
- Praca w pełni automatyczna, wszystko sprawdzane kilka razy; najpierw plan do zatwierdzenia.

## 2026-10-11 20:45 — plan zatwierdzony, start prac

Właściciel: „Zapisz w katalogu plan i zaczynaj prace! pełną siłą!” — plan z `alfa/docs/plan/16–22` (wersja 0.0.29)
zatwierdzony. Kolejność: P1 (rozmowa i mowa) → P2 (modele) → dalej wg dokumentu 16, praca automatyczna.
Dopisane tego dnia grupy: P12 (agentki programistki, przycisk „Pełne uprawnienia” jak w Claude Code/Codex),
P13 (dyski sieciowe i chmurowe, kopie), P14 (obowiązki codzienne — poczta itd.).
