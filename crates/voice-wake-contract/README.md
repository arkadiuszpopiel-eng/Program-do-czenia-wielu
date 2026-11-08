# voice-wake-contract

Kontrakt aktywacji (docs/modules/voice-wake/SPEC.md): trait `Wake` (`configure`, `pump` — zdarzenia skrótów,
`handle(WakeInput)`, `addressed(text)`, `mic_state`, `is_listening`).

Wspólny deterministyczny **`WakeMachine`**: PTT (wciśnięcie/puszczenie z `HotkeyPort` — hook `WH_KEYBOARD_LL`;
autopowtórzenia ignorowane), przełącznik, przycisk/Spacja w UI, wyciszenie (blokuje PTT), „nie przeszkadzać”
(nie wyłącza PTT), stan mikrofonu `Off/Listening/Hearing/Processing/Muted`, okno administratora na pierwszym planie
(`blocked_elevated_foreground` raz na epizod), adresowanie z transkryptu (`personas-contract`: imię wygrywa,
inaczej Dyrygentka obsady). `WakeCfg::default()`: PTT `Ctrl+Shift+Space`, przełącznik `Ctrl+Shift+M` (walidacja
reguły AltGr i kill-switcha). Zdarzenia `voice.wake.*`.

**v1 (F5) — słowa wywoławcze** (domyślnie wyłączone, `WakeCfg::wake_words = None`):
- `WakeWordCfg` (`from_personas` — frazy z `Persona::wake_phrases`, także imiona z Kreatora; `validate`: ≥ 3 sylaby,
  ≤ 40 znaków, różne, próg w (0, 1); „zawsze słucham” → `NotAvailable`, poza v1), `KwsParams` (histereza 0,15,
  `min_hits` 2, okno odporności 2 s, limit ciszy po wybudzeniu 6 s, bufor 2 s, bramka energii z pre-rollem 400 ms).
- `WakeWordListener` — bufor pierścieniowy o stałej pojemności (jedyne miejsce audio przed wykryciem, bez publicznego
  odczytu), bramka energii (`voice-vad-contract::EnergyDetector`), model przez trait `KeywordScorer`, detektor
  `WakeWordDetector`, bramka właściciela `OwnerCheck` (fail-closed), `WakeWordTrigger` (audio frazy dopiero po
  wykryciu), `set_suspended` (DND/wyciszenie/słuchanie czyści bufor).
- Automat: `WakeInput::WakeWord { persona, phrase, at_ms }` → `ListenStart { addressed: Some(persona), source:
  WakeWord }` + `Addressed`; `WakeInput::Tick` zamyka sesję po ciszy (`false_alarm_suspected`, gdy nie padło ani
  słowo); wykrycia przy wyłączonych słowach, DND, wyciszeniu albo trwającym słuchaniu → `voice.wake.word_ignored`.
- `eval` — FAR/dzień i FRR z przeglądem progów (ten sam detektor), progi F5-05/06 i minimalne liczności.
