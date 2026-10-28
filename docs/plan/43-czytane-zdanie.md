# 43 — Wyróżnianie czytanego zdania (projekt, bez kodu)

Stan na **2026-10-28**, wersja 0.0.75. Propozycja 2 z raportu nocnego [`42`](42-raport-2026-10-28-noc.md):
gdy Alfa czyta odpowiedź na głos, zdanie, które właśnie słychać, jest wyróżnione w treści wiadomości, a litery
wylatujące z ust awatara pochodzą z tego zdania. Przyda się w etapie K (nauka czytania —
[`41`](41-tryb-dla-dzieci.md)). Dokument opisuje, jak to zrobić; kod czeka na decyzję właściciela.

## 1. Jak jest dziś (sprawdzone w kodzie)

| Element                                | Gdzie                                                                                                               | Co robi                                                                                                                                  |
| -------------------------------------- | ------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------- |
| Przycisk „Przeczytaj na głos”          | `MessageActions.svelte` → polecenie `turns_read_aloud`                                                              | tylko przy wiadomościach agentek                                                                                                         |
| Rdzeń polecenia                        | `app-core/src/commands/turn_intents.rs` → `lib_markdown::to_spoken_text`                                            | zdejmuje znaczniki, kod zastępuje „(kod na ekranie)”, scala odstępy; potem `VoicePort::read_aloud`                                       |
| Synteza                                | `app-modules/src/voice.rs` (`read_aloud`), `voice-tts-impl` → `split_sentences` (`voice-tts-contract/src/text.rs`) | tekst dzielony na zdania; każdy kawałek (`TtsChunk`) ma numer i znaczniki słów (`WordMark`), ale bez położenia w tekście wiadomości      |
| Zdarzenia do okna                      | tylko `AlfaEvent::SpeechLevel { agent, level }` (`app-api/src/dto/events.rs`)                                       | poziom głosu w rytm odtwarzania — porusza ustami awatara                                                                                 |
| Litery z ust                           | `MessageItem.svelte` → `Avatar` z `words={turn.text}` tylko podczas **pisania** odpowiedzi                         | nie są związane z czytaniem na głos                                                                                                      |
| Rozmowa głosowa (czytanie automatyczne) | `voice-pipeline-impl` (`SentenceChunker`, `speech/play.rs`)                                                         | osobna droga; zdarzenia `TtsChunkQueued` / `TtsWordMarks` zostają w automacie dialogu, do okna nie trafiają                               |
| Czytanie zaznaczenia (globalne)        | `voice-readaloud` (`Segment { start, end }`, `ReadAloudEvent::Segment`)                                             | ma podział z położeniem znaków, ale tylko dla tekstu z zaznaczenia; w oknie widać numer odcinka w karcie czytania, nie wyróżnienie tekstu |

Wniosek: zdania już istnieją w syntezie, ale okno nie wie, które zdanie właśnie słychać.

## 2. Projekt

**Rdzeń (a):**

- Nowe zdarzenie `AlfaEvent::SpeechSegment { turn_id, index, of, text }` w `app-api/src/dto/events.rs`
  (+ wygenerowane typy TS). `text` to tekst mówiony zdania — bez treści spoza wiadomości, którą okno i tak ma.
- `app-modules/src/voice.rs` (`read_aloud`): wysyła zdarzenie, gdy kawałek **zaczyna brzmieć** — z tym samym
  opóźnieniem względem odtwarzania co `SpeechMeter` (inaczej wyróżnienie wyprzedza głos o długość bufora).
- `turn_id` przechodzi przez `VoicePort::read_aloud` (`app-api/src/ports/voice.rs`, `app-voice/src/port.rs`,
  `turn_intents.rs`); atrapy w testach `app-core` dostają nowy parametr.
- Druga faza: to samo w rozmowie głosowej (`voice-pipeline-impl/src/speech/play.rs` → most w `app-voice`).

**Okno (b):**

- `apply-batch.ts`: stan `{ turnId, text }`; czyści go `SpeechLevel` bez agentki albo zatrzymanie czytania.
- `MessageItem.svelte`: efekt znajduje zdanie w tekście wiadomości (węzły tekstowe, odstępy znormalizowane) i
  wyróżnia je przez **CSS Custom Highlight API** (`CSS.highlights`) — bez zmiany drzewa DOM, więc HTML z Rusta
  (pulldown-cmark + ammonia) zostaje nietknięty. WebView2 (Chromium) to obsługuje.
- Kolor wyróżnienia z tokenów stylu (każdy styl × motyw), bez animacji przy ograniczonym ruchu.
- `Avatar`: `speaking` także podczas czytania tej wiadomości, `words` = czytane zdanie — litery z ust pochodzą
  z tego, co słychać.

**Dopasowanie tekstu:** zdanie mówione różni się od wyświetlanego (kod zastąpiony, znaczniki zdjęte). Dopasowanie
po słowach z tolerancją; gdy się nie uda (np. zdanie w bloku kodu), wyróżnienia nie ma — czytanie działa dalej.

## 3. Obszary zastrzeżone

Żadna z tych zmian nie dotyka `crates/core-*`, `safety-broker`, `broker-ui`, watchdoga ani polityk.
Zdarzenie idzie przez `app-api`, nie przez magistralę `core-bus` (zmiana tam wymagałaby przeglądu człowieka).

## 4. Sprawdzenie (po wdrożeniu)

- Test rdzenia: kolejność i liczba zdarzeń `SpeechSegment` na atrapie syntezy, opóźnienie względem odtwarzania.
- Test przeglądarkowy na atrapie: wyróżnienie przesuwa się zdanie po zdaniu we wszystkich stylach i motywach,
  znika po zatrzymaniu; przy ograniczonym ruchu bez animacji.
- Test w prawdziwym oknie (WebView2) i odsłuch na sprzęcie (`verify sprzet`) — **za dnia**, nie w nocy.

## 5. Do decyzji właściciela

1. Czy wprowadzać (szacunek: jedna seria — rdzeń, okno, testy)?
2. Czy od razu w rozmowie głosowej, czy najpierw tylko przycisk „Przeczytaj na głos”?
