# voice-s2s — SPEC (v0, F5: kontrakt i fake w repo; adapter chmurowy później)

## Cel
Natywny speech-to-speech w chmurze jako „tryb szybkiej rozmowy” (PLAN §6.2, §6.3 profil C, §6.5; VOICE.md): audio mikrofonu → dostawca (OpenAI Realtime / Gemini Live) → audio odpowiedzi, z przerwaniem (barge-in) obcinającym odpowiedź **po stronie dostawcy** do miejsca, które użytkownik usłyszał. Głosy = presety dostawcy przypisane do agentek (nie własne biblie głosu). Lokalnego S2S dla PL brak — tryb opcjonalny, za tagiem prywatności.

## Fala i priorytet
F5, P1 (opcjonalny). Odsłuch jakości PL — bramka ludzka #5.

## Kontrakt (Rust, `voice-s2s-contract`)
```rust
pub enum S2sProvider { OpenAiRealtime, GeminiLive }   // interruption(): NativeTruncate | AppendNote
pub struct S2sCfg { provider, account /* accounts-hub; klucz nigdy w konfiguracji */, model, voices: Vec<(PersonaId, String)>,
                    privacy: PrivacyTag, sample_rate /* 16/24 kHz */, max_session_minutes /* 1–120 */ }   // validate(&persona)
#[async_trait] pub trait S2sClient: Send + Sync {
    async fn connect(&self, cfg: &S2sCfg, persona: &PersonaId, instructions: &str) -> Result<Box<dyn S2sSession>, S2sError>;
}
#[async_trait] pub trait S2sSession: Send {
    async fn send_audio(&mut self, frame: &Frame) -> Result<(), S2sError>;   // mono, sample_rate z cfg; każde → AudioSent
    async fn commit_turn(&mut self) -> Result<(), S2sError>;                 // pusty bufor → Provider
    async fn cancel_response(&mut self) -> Result<(), S2sError>;             // potem żadne AudioDelta tego elementu
    async fn truncate(&mut self, item: &ItemId, audio_end_ms: u32) -> Result<(), S2sError>;   // przycięte do dostarczonego audio
    fn poll(&mut self) -> Vec<S2sEvent>;   // UserSpeechStarted | AudioDelta { item, audio } | Transcript { role, text, is_final } | ResponseDone { item } | Error
    fn take_bus_events(&mut self) -> Vec<S2sBusEvent>;
    async fn close(&mut self);             // idempotentne, jedno SessionClosed; potem → Closed
}
pub fn truncate_point(played_samples: u64, sample_rate: u32, device_latency: Duration) -> u32;   // usłyszane ms
pub enum S2sError { PrivacyBlocked, InvalidConfig(String), Closed, UnknownItem(String), Unsupported, Provider(String) }
```
Zdarzenia magistrali (bez audio i treści): `voice.s2s.session_started` (dostawca, model, agentka), `voice.s2s.audio_sent` (ms — ekran „co poszło do chmury”), `voice.s2s.truncated` (element, `audio_end_ms`), `voice.s2s.session_closed` (łącznie wysłane ms).

## Zależności
`core-bus-contract`, `personas-contract`, `providers-contract` (`PrivacyTag`, `InterruptionRendering`), `voice-audio-contract` (`Frame`). Adapter: `accounts-hub` (klucz z Credential Manager), WebSocket/WebRTC.

## Niezmienniki
- Sesja z tagiem `Private` **nigdy nie łączy się** z chmurą (`PrivacyBlocked` przed jakimkolwiek połączeniem; test: 0 połączeń, 0 ms audio).
- Każda porcja wysłanego audio jest raportowana (`audio_sent`); audio w innym formacie niż konfiguracja jest odrzucane i niewysłane.
- Przerwanie: `cancel_response` + `truncate(item, truncate_point(odtworzone próbki, częstotliwość, opóźnienie urządzenia))` — OpenAI Realtime `conversation.item.truncate` (`InterruptionRendering::NativeTruncate`): historia u dostawcy obcięta do usłyszanego miejsca, **bez notki**. Dostawca bez natywnego obcięcia (Gemini Live) → `Unsupported` → potok dopisuje notkę (`AppendNote`) w historii append-only.
- Historia rozmowy Alfy pozostaje append-only: tura asystentki zapisywana raz, z usłyszanym prefiksem.
- Klucze API nigdy w konfiguracji ani logach (tylko `account` → `accounts-hub`).
- Limit długości sesji (`max_session_minutes`) — koszt; przekroczenie zamyka sesję (adapter).

## Zdolności / uprawnienia
`net.connect` do hosta dostawcy (adapter); mikrofon i głośnik przez `scheduler-lite` (jak potok).

## Izolacja
`process` (adapter chmurowy), `lazy` — startuje tylko w trybie szybkiej rozmowy.

## Budżet zasobów
Adapter: RAM ≤ 30 MB, CPU ≤ 3 % (kodowanie PCM16/base64, resampling 16→24 kHz); opóźnienie do pierwszego audio — mierzone w Voice Lab (profil C).

## Konfiguracja (klucze TOML)
`[voice.s2s] enabled = false`, `provider = "openai_realtime"`, `account = "<id konta>"`, `model = "<model>"`, `voices = { alfa = "<preset>", … }`, `sample_rate = 24000`, `max_session_minutes = 30`.

## Wkład do UI
Przełącznik „Szybka rozmowa (chmura)” w panelu Głos z ostrzeżeniem o wysyłce audio; niedostępny w sesji prywatnej; licznik wysłanego audio; wybór presetu głosu per agentka.

## Testy akceptacyjne
- `ACC-F5-voice-s2s-01` (CI): testy kontraktowe na atrapie — sesja prywatna nie łączy się; tura: `audio_sent` = suma wysłanych ms, odpowiedź strumieniowana (≥ 2 porcje), transkrypcja, `ResponseDone`; barge-in: brak audio po `cancel_response`, `truncate` → `Truncated` z usłyszanym ms (przycięte do dostarczonego); nieznany element → `UnknownItem`; Gemini Live → `Unsupported`; zamknięcie idempotentne.
- `ACC-F5-voice-s2s-02` (self-hosted, bramka #5): odsłuch PL presetów, opóźnienie do pierwszego audio, poprawność obcięcia historii po przerwaniu.

## Fake
`voice-s2s-fake`: `FakeS2sClient` — skryptowane odpowiedzi (`script_reply`), strumieniowanie po 100 ms na `poll`, VAD serwera (`UserSpeechStarted` przy głośnym audio w trakcie odpowiedzi), historia dostawcy obcinana proporcjonalnie do usłyszanego audio (na granicy słowa, bez notki); liczniki połączeń i wysłanego audio; przechodzi testy kontraktowe.

## Otwarte pytania
- Transport: WebSocket (serwer) vs WebRTC (niższe opóźnienie, AEC przeglądarki niedostępne w Tauri) — spike w adapterze.
- Narzędzia (function calling) w sesji S2S — wymagają tej samej ścieżki Brokera co czat; poza v0.
- Gemini Live: zachowanie przy przerwaniu (czy usuwa nieusłyszany tekst) — do sprawdzenia; dziś `AppendNote`.

## Implementacja w aplikacji (F5, `app-voice`)
- Bez adaptera chmurowego: `voice_features` zwraca `s2s.available = false` z powodem „wymaga klucza API dostawcy (OpenAI Realtime) — adapter w przygotowaniu”; panel Głos pokazuje kartę „Szybka rozmowa (chmura) — Wymaga klucza” (w sesji prywatnej nigdy się nie łączy).
