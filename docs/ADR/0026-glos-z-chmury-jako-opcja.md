# ADR 0026 — Głos agentek z chmury jako opcja obok głosu lokalnego

| Pole | Wartość |
|---|---|
| Status | Proponowany — czeka na decyzję właściciela |
| Data | 2026-10-10 |
| Powiązane | zgłoszenie właściciela z 2026-10-09 („głos sztywny i niemiły”); ADR 0011 (silniki mowy v0), ADR 0024 (licencja silnika mowy); `docs/modules/voice-tts/SPEC.md`; `docs/plan/13-podsumowanie-i-dalsze-kroki.md` |
| Zmienia | ADR 0011 w części „chmura w kolejnej fali” — proponuje konkretny, opcjonalny adapter |

## Kontekst

Wszystkie cztery agentki mówią dziś jednym głosem lokalnym (Piper „Gosia”), przestrajanym na cztery. Wersja
0.0.14 dodała strojenie (styl, tempo, pauzy, siła różnic), a 0.0.17 głośność — to poprawia brzmienie, ale nie da
czterech naturalnych, wyraźnie różnych głosów. Są dwie drogi:

| Droga | Co daje | Czego wymaga |
|---|---|---|
| A. Inny głos lokalny | wszystko zostaje na komputerze; działa bez sieci | pobranie silnika i głosów, sprawdzenie licencji (ADR 0024), pomiar jakości po polsku i zużycia karty obok modelu rozmowy |
| B. Głos z chmury | głosy wysokiej jakości od razu, różne dla każdej agentki, bez obciążania karty | wysyłanie **tekstu wypowiedzi** do dostawcy; koszt za znak; sieć |

W kodzie jest już miejsce na drogę B: typ `TtsEngine::Cloud` i łańcuch głosów z zapasem — serwis mowy próbuje
kolejnych ogniw, a sesja prywatna odrzuca ogniwo chmurowe (`PrivacyBlocked`). Brakuje adaptera, dostępu do klucza
konta i ustawień. Sesja nie zrobiła tego bez właściciela, bo adapter używa klucza API i wysyła treść poza komputer.

## Decyzja (propozycja)

1. **Głos z chmury jest opcją, domyślnie wyłączoną.** Głos lokalny zostaje domyślny i jest zawsze zapasem: brak
   sieci, błąd dostawcy albo przekroczony limit kosztów = ta sama wypowiedź głosem lokalnym, bez przerwy rozmowy.
2. **Nigdy w sesji prywatnej** — tam mówi wyłącznie głos lokalny (mechanizm już istnieje).
3. **Tylko przez konto, które właściciel już dodał** w „Konta i klucze API”. Klucz pobierany tą samą drogą co dla
   modeli rozmowy (z Windows Credential Manager, bez zapisu gdziekolwiek indziej); wybór konta i głosu per agentka
   w zakładce „Głos”.
4. **Do dostawcy trafia tylko tekst zdania do wypowiedzenia** (po normalizacji), nigdy nagranie mikrofonu ani
   historia rozmowy. Trasa jest wpisana do rejestru zgodności jak trasy modeli.
5. **Koszt mowy liczy się do limitów** z „Koszty i limity” i jest widoczny w szczegółach kosztów.
6. **Przy każdej agentce widać, którym głosem mówi** (lokalny / chmura: dostawca), a odsłuch w Ustawieniach
   pozwala porównać oba przed włączeniem.

## Opcje dla właściciela

| Pytanie | Propozycja | Alternatywy |
|---|---|---|
| Którą drogą iść | B jako opcja teraz, A po rozstrzygnięciu ADR 0024 | tylko A · tylko B |
| Który dostawca pierwszy | ten, do którego właściciel ma już klucz | inny dostawca z lepszym polskim głosem — po próbie |
| Kiedy chmura, kiedy lokalnie | chmura w profilach „Hybryda” i „Chmura”, lokalnie w „Lokalny” | zawsze chmura, gdy włączona |

## Konsekwencje

- Nowy adapter w `voice-tts-impl` (żądanie HTTP przez istniejący klient z `lib-openai-compat` i `lib-netguard`),
  źródło klucza z `accounts-hub`, wpis trasy w rejestrze zgodności, ustawienia `voice.cloud_*`, test na serwerze
  zastępczym (bez prawdziwego klucza), pomiar opóźnienia pierwszego dźwięku.
- Znaczniki słów (ruch ust awatara już ich nie potrzebuje — korzysta z poziomu dźwięku) mogą być szacowane.
- Parametry interfejsów dostawców (modele, nazwy głosów, formaty dźwięku, ceny) **nie były sprawdzane w źródłach**
  — sprawdzenie jest pierwszym krokiem zadania, z wpisem w `docs/vendor/`.
- Zadanie w roadmapie: C1.1a (po decyzji).
