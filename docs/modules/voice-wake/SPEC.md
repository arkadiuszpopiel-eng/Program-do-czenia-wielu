# voice-wake — SPEC (v0 zaimplementowany w F2, v1 — słowa wywoławcze — w F5)

## Cel
Aktywacja słuchania i adresowanie: v0 — push-to-talk / przełącznik (hook `WH_KEYBOARD_LL` przez `platform-windows`) + adresowanie po imieniu z transkryptu („Delta, …"); v1 — opcjonalne słowa wywoławcze „Hej Alfa/Beta/Gama/Delta" (własny trening; openWakeWord tylko EN), tryb „zawsze słucham" z bramką właściciela, „nie przeszkadzać" (PLAN §6.2, §7.3, §16.2).

## Fala i priorytet
F2: v0 (PTT/toggle, imię z transkryptu). F5: v1 (wake words po spełnieniu FAR/FRR, bramka `voice-speaker`). P0 (v0).

## Kontrakt (Rust, `voice-wake-contract`)
```rust
pub struct WakeCfg { pub ptt_key: Option<Hotkey> /* Ctrl+Shift+Space */, pub toggle_key: Option<Hotkey> /* Ctrl+Shift+M */,
                     pub name_addressing: bool, pub wake_words: Option<WakeWordCfg> /* v1 (F5) — w v0 odrzucane */ }
pub enum WakeInput { Key { id, pressed }, UiPtt { pressed }, UiToggle, Vad { speech }, Processing { busy }, Transcript { text },
                     SetMuted { muted }, SetDnd { on }, ElevatedForeground { elevated } }
pub enum WakeEvent { ListenStart { addressed, source }, ListenStop { source }, Addressed { persona, by_name }, BlockedElevatedForeground,
                     Dnd { on }, MicState { state: MicState /* Off | Listening | Hearing | Processing | Muted */ } }
pub trait Wake: Send {
    fn configure(&mut self, cfg: WakeCfg) -> Result<(), WakeError>;  fn pump(&mut self) -> Vec<WakeEvent>;   // skróty z HotkeyPort
    fn handle(&mut self, input: WakeInput) -> Vec<WakeEvent>;  fn addressed(&self, text: &str) -> Option<PersonaId>;
    fn mic_state(&self) -> MicState;  fn is_listening(&self) -> bool;
}
// Wspólne: WakeMachine (deterministyczny automat; adresowanie przez personas-contract::resolve_addressee).
// Wspólne: MicArbiter (dzierżawa mikrofonu w scheduler-lite: Holder::User, Priority::UserSpeech; apply/apply_now)
//          i lease_now (dzierżawa bez czekania — dla wątku przetwarzania potoku, który nie może blokować kroku).
// v1 (F5): WakeInput::{WakeWord { persona, phrase, at_ms }, Tick { now_ms }};
//          WakeEvent::{FalseAlarmSuspected { persona, phrase }, WakeWordIgnored { persona, reason }}
pub trait KeywordScorer: Send { fn labels(&self) -> &[String]; fn push(&mut self, s: &[f32]) -> Result<Vec<KwsScores>, WakeError>; fn reset(&mut self); }
pub trait OwnerCheck: Send { fn is_owner(&mut self, audio: &[f32]) -> Option<bool>; }   // None = nie wiadomo → odrzucenie
pub struct WakeWordListener;  // new(cfg, params, scorer) · with_owner_check · push(16 kHz) -> Option<WakeWordTrigger> · set_suspended
pub struct WakeWordDetector;  // próg + histereza + min_hits + okno odporności; etykieta → persona (normalize_phrase)
pub mod eval { evaluate, sweep, recommend, default_thresholds }  // FAR/dzień, FRR, przegląd progów
```
Zdarzenia: `voice.wake.listen_start/stop`, `voice.wake.addressed`, `voice.wake.blocked_elevated_foreground` (okno admina — hook nie działa), `voice.wake.false_alarm_suspected` (v1), `voice.wake.word_ignored` (v1: wyłączone/wyciszone/DND/już słucha), `voice.wake.dnd`, `voice.wake.mic_state`.

## Zależności
`core-bus/config/log-contract`, `platform-windows-contract` (hook PTT, skróty), `voice-stt-contract` (imiona z transkryptu), `personas-contract` (imiona, frazy), `voice-dialog-contract`, `voice-speaker-contract` (F5), `device-profile-contract`/`shell-integration-contract` (DND).

## Niezmienniki
- PTT zgłasza puszczenie klawisza (hook, nie zwykły skrót); przy oknie administratora na pierwszym planie bez helpera `uiAccess` PTT nie działa — UI komunikuje to wprost.
- Zwrot po imieniu zawsze wygrywa; bez imienia odpowiada agentka z rolą Dyrygentki.
- Wake words włączane dopiero po spełnieniu FAR ≤ 1/dzień i FRR ≤ 5% (F5); domyślnie wyłączone.
- Tryb „zawsze słucham" wymaga bramki właściciela (`voice-speaker`); bez niej niedostępny.
- DND wyłącza wake/proaktywną mowę, nie wyłącza PTT.
- Wskaźnik prywatności w zasobniku przy otwartym mikrofonie.

## Zdolności / uprawnienia
Brak.

## Izolacja
`inproc`, `lazy` (v0 lekki; v1 KWS na CPU z potokiem głosu).

## Budżet zasobów
v0: RAM ≤ 1 MB, reakcja PTT ≤ 10 ms; v1: KWS CPU ≤ 3% rdzenia, RAM ≤ 30 MB.

## Konfiguracja (klucze TOML)
`[voice.wake] mode = "ptt"`, `ptt_key = "Space"` (poza polem tekstowym), `toggle_key = "Ctrl+Shift+M"`, `name_addressing = true`; v1: `wake_words = false`, `phrases = ["Hej Alfa", "Hej Beta", "Hej Gama", "Hej Delta"]`, `always_on = false`.

## Wkład do UI
Przycisk mikrofonu i stany (wyłączony/słucha/…), przytrzymanie Spacji = mów, `Ctrl+Shift+M`, Ustawienia → Głos → Słowa wywoławcze, komunikat „okno administratora na wierzchu".

## Testy akceptacyjne
- `ACC-F2-voice-wake-01`: PTT start/stop ≤ 10 ms od klawisza (hook), 100/100 na runnerze; toggle działa z ukrytym oknem.
- `ACC-F2-voice-wake-02`: adresowanie po imieniu z transkryptu ≥ 98% na zestawie ≥ 100 wypowiedzi z korpusu.
- `ACC-F5-voice-wake-03`: FAR ≤ 1/dzień na ≥ 24 h tła PL (TV, podcasty); FRR ≤ 5% na ≥ 200 pozytywach właściciela.

## Fake
`voice-wake-fake`: zdarzenia PTT/adresowania ze skryptu na wirtualnym zegarze.

## Otwarte pytania
- Trening KWS „Hej …" (dane syntetyczne + korpus) — spike (h); frazy 3–4-sylabowe (ryzyko krótkich imion).

## Decyzje v0 (F2)
- Globalny PTT domyślnie `Ctrl+Shift+Space` (reguła `Hotkey::validate` wymaga modyfikatora); Spacja bez modyfikatora — tylko w oknie aplikacji (`WakeInput::UiPtt`).
- Mikrofon jako zasób wyłączny `scheduler-lite` na czas słuchania (`MicArbiter`: `Holder::User`, `Priority::UserSpeech`).
- Wyciszenie blokuje PTT; DND nie (SPEC). Niepoprawna konfiguracja nie zmienia zarejestrowanych skrótów.

## Decyzje v1 (F5)
- **Domyślnie wyłączone** (`wake_words = None`); tylko lokalnie, bez chmury. `always_on = true` → `WakeError::NotAvailable` (wymaga weryfikacji właściciela na każdej wypowiedzi — poza v1).
- Frazy: z `Persona::wake_phrases` („Hej Alfa/Beta/Gama/Delta” + imiona z Kreatora, `WakeWordCfg::from_personas`); walidacja: 1–16 fraz, różne po `normalize_phrase`, ≥ 3 sylaby, ≤ 40 znaków.
- Strojenie startowe (`KwsParams`, do kalibracji na korpusie F5-05/06): próg 0,8, histereza 0,15, `min_hits` 2, okno odporności 2 s, bufor pierścieniowy 2 s (0,5–4 s), bramka energii (`EnergyDetector`) z pre-rollem 400 ms i podtrzymaniem 600 ms, brak mowy 6 s po wybudzeniu → `false_alarm_suspected`.
- Prywatność (testowane): audio przed wykryciem żyje tylko w buforze nasłuchu (stała pojemność, bez publicznego odczytu, `Debug` bez próbek); wyciszenie/DND/trwające słuchanie czyści bufor. W potoku (`Pipeline::arm_wake_words`) przed wykryciem 0 zdarzeń VAD/STT/transkrypcji i 0 audio na magistrali; ramki przechodzą AEC (agentka nie budzi się własnym głosem).
- Bramka właściciela (`owner_gate`, opcjonalna): `OwnerCheck` na audio frazy; `None` (brak profilu, za krótko) = odrzucenie (fail-closed).
- Model: `voice-wake-impl::kws::load_scorer(<model>.kws.json)` — manifest `alfa-kws-v1` z SHA-256 każdego pliku ONNX, inferencja `tract-onnx` 0.23.8; rodzaje `log_mel` (log-mel Kaldi z `voice-dsp-contract::fbank`, własny trening PL) i `openwakeword` (3 etapy: melspektrogram → embedding 96 cech/80 ms → klasyfikator frazy). Modeli nie ma w repo (README `voice-wake-impl`: źródła i licencje); KWS sherpa-onnx (transducer) poza v1.
- Pomiar: `alfa-wake-eval` (`check`/`run`/`freeze`/`schema`), format `evals/F5/voice/README.md` §1; próbki syntetyczne w CI, korpus poza git.
