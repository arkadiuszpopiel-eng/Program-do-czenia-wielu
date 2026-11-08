# voice-dictation — SPEC (v1, F5: kontrakt, fake i impl w repo)

## Cel
Dyktowanie do dowolnej aplikacji (PLAN §6.2, §7.3, §16.2 F5; ACCEPTANCE F5-10): strumieniowy STT → normalizacja odwrotna PL (komendy interpunkcji, liczby tylko jednoznaczne) → wpisanie przez `InputPort` (`SendInput` Unicode) **wyłącznie do okna, które było na pierwszym planie w chwili startu**. Nigdy do okien Alfy/Brokera ani pól haseł. „cofnij to”, push-to-talk i przełącznik.

## Fala i priorytet
F5, P1 (po `platform-windows` v1.5: SendInput tekstu, UIA).

## Kontrakt (Rust, `voice-dictation-contract`)
```rust
pub trait Dictation: Send {
    fn start(&mut self, mode: DictationMode /* PushToTalk | Toggle */, now_ms: u64) -> Result<DictationStatus, DictationError>;
    fn stop(&mut self) -> DictationStatus;
    fn on_final(&mut self, text: &str, now_ms: u64) -> Result<DictationStatus, DictationError>;   // komenda sterująca albo fraza
    fn tick(&mut self, now_ms: u64) -> DictationStatus;      // obserwacja pierwszego planu: pauza/wznowienie, ponowienie zaległego tekstu
    fn undo_last(&mut self, now_ms: u64) -> Result<DictationStatus, DictationError>;
    fn status(&self) -> DictationStatus;  fn take_events(&mut self) -> Vec<DictationEvent>;
}
pub fn normalize(text: &str, ctx: &mut TextContext) -> String;     // komendy interpunkcji PL + numbers (tylko jednoznaczne)
pub fn control_command(text: &str) -> Option<ControlCommand /* Undo | Stop */>;   // tylko cała wypowiedź
pub struct DictationMachine;   // rdzeń bez I/O (cel, fazy, kolejka tekstu, okno „cofnij”) wspólny dla -impl i -fake
pub enum DictationPhase { Idle, Active, Paused(PauseReason /* FocusChanged | UserTyping */) }
pub enum RefuseReason { NoForeground, ProtectedTarget, ElevatedTarget, PasswordField }
```
Zdarzenia (bez treści — liczby znaków i nazwa aplikacji): `voice.dictation.started`, `typed`, `paused`, `resumed`, `stopped`, `refused`, `undone`, `undo_unavailable`, `newline_blocked`.

## Zależności
`core-bus-contract`, `platform-contract` (`DesktopPort`, `UiaPort`, `InputPort`, `TargetGuard`), `personas-contract` (`fold`), `voice-stt-contract`, `voice-vad-contract`, `voice-audio-contract`, `scheduler-lite-contract` (mikrofon).

## Niezmienniki
- Cel = okno na pierwszym planie w chwili startu; inne okno na pierwszym planie → pauza (tekst czeka), powrót → wznowienie; zniknięcie celu → koniec.
- Odmowa startu: okno Alfy/Brokera/nieznanego procesu (`TargetGuard`), okno administratora (UIPI), fokus w polu hasła (UIA `IsPassword`) — a gdy fokusu nie da się ustalić, dowolne pole hasła w oknie albo błąd UIA = odmowa (**fail-closed**). Pole hasła sprawdzane też przed każdą frazą.
- Tekst w porcjach ≤ 8 jednostek UTF-16; każda porcja = jedna atomowa paczka `SendInput` (strażnik celów, fizyczne wejście i fokus sprawdzane przed paczką) — po przerwaniu wiadomo dokładnie, co wpisano.
- W terminalach „nowa linia” nie naciska Enter (`newline_blocked`; zastępuje spacją).
- „cofnij to” usuwa tylko ostatnią frazę (Backspace), ≤ 30 s i bez zmiany okna; inaczej `undo_unavailable`.
- Fizyczne wejście użytkownika ma pierwszeństwo (pauza, wznowienie po 1,5 s ciszy klawiatury/myszy).
- Dyktowany tekst nie trafia na magistralę, do logów ani do pamięci.
- Mikrofon jako zasób wyłączny `scheduler-lite` (`Holder::User`); zajęty → odmowa; odebranie dzierżawy (kill-switch) kończy sesję.

## Zdolności / uprawnienia
`input.synthesize` (tylko tekst Unicode i Backspace do okna celu); odczyt UIA (`IsPassword`, fokus).

## Izolacja
`inproc`, `lazy`.

## Budżet zasobów
RAM ≤ 4 MB, CPU ≤ 1 % (STT liczy `voice-stt`); fraza wpisana ≤ 50 ms + 20 ms na porcję.

## Konfiguracja (klucze TOML)
`[voice.dictation] capitalize_start = true`, `block_enter_in_terminals = true`, `undo_window_ms = 30000`, `max_phrase_chars = 2000`, `idle_stop_ms = 60000`; skrót (PTT/przełącznik) w `[voice.wake]`.

## Wkład do UI
Pigułka dyktowania (aplikacja docelowa, aktywne/wstrzymane i dlaczego, liczba znaków), komunikaty odmowy („pole hasła — dyktowanie wyłączone”), przycisk „Cofnij”.

## Testy akceptacyjne
- `ACC-F5-voice-dictation-01` (CI): `evals/F5/voice/dictation-cases.json` — normalizacja 30/30 (interpunkcja, liczby jednoznaczne i niejednoznaczne, „dosłownie”).
- `ACC-F5-voice-dictation-02` (CI, `platform-fake`): property 200 losowych scenariuszy (zmiany fokusu, okna chronione, hasła, wejście użytkownika) → **0 wpisów do okien chronionych, pól haseł i poza oknem celu**; kontraktowe na atrapie i impl; runner PTT/przełącznik na wirtualnym zegarze.
- `ACC-F5-voice-dictation-03` (self-hosted, F5-10): 5 aplikacji (Notatnik, WordPad, przeglądarka, VS Code, Word), ≥ 95 % zgodności tekstu.

## Fake
`voice-dictation-fake`: `FakeDictation` — `DictationMachine` na wirtualnym pulpicie `FakeWindows` (okna, fokus, pola haseł, zapis wpisów); przechodzi testy kontraktowe.

## Otwarte pytania
- Wpisywanie przez UIA `ValuePattern` zamiast `SendInput` w aplikacjach z autouzupełnianiem (VS Code) — po pomiarze F5-10.
- Odczyt kontekstu (znak przed kursorem) z UIA `TextPattern` dla wielkiej litery/spacji — dziś stan sesji.

## Decyzje v1 (F5)
- Liczby → cyfry tylko gdy jednoznaczne: złożenie ≥ 2 słów albo jedno słowo 11–99; zostają słowami 0–10, samotne „sto/tysiąc/milion”, formy odmienione i porządkowe; przecinek z STT między liczebnikami rozdziela liczby („dwadzieścia, trzy” → „20, trzy”).
- Interpunkcja dodana przez STT zostaje, komenda ją zastępuje (bez podwójnych znaków).
- `DictationRunner` (impl): resampling do 16 kHz, PTT = jedna wypowiedź na przytrzymanie, przełącznik = VAD + pre-roll 300 ms; „koniec dyktowania” zamyka mikrofon.

## Implementacja w aplikacji (F5, `app-voice`)
- Komenda `voice_dictation` (`start`/`stop`/`toggle`/`undo`, `save_profile`/`remove_profile`) i skrót globalny `Ctrl+Alt+D` (powłoka, przełącznik; D nie jest literą polską — reguła AltGr; rejestr skrótów UI wykrywa konflikty, zajęty skrót → toast). PTT dyktowania z UI pominięte: okno Alfy na pierwszym planie jest chronione (przycisk ma odliczanie 3 s).
- `DictationRunner` (STT `whisper-server` i VAD jak rozmowa) + `DictationService` na portach computer use (`GuiPorts`: strażnik okien Alfy/Brokera). Odmowa (pole hasła, okno chronione/admina) → komunikat PL/EN, nic nie jest wpisywane.
- Profile per aplikacja (`voice.dictation_profiles`, nazwa pliku procesu): wielka litera na początku, „nowa linia” bez Entera w terminalach — dobierane po oknie na pierwszym planie w chwili startu.
- Podgląd = ostatnia fraza STT tylko w widoku okna Alfy (czyszczony po sesji); zdarzenia na magistrali bez treści. Na czas dyktowania rozmowa głosowa i nasłuch słów są wstrzymane, a tury głosowe nie idą do modelu.
- Testy: `crates/app-voice/tests/f5_dictation.rs` (pole hasła = odmowa i 0 paczek wejścia, wpisanie do Notatnika i stop, profil aplikacji).
