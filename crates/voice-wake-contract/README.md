# voice-wake-contract

Kontrakt aktywacji v0 (docs/modules/voice-wake/SPEC.md): trait `Wake` (`configure`, `pump` — zdarzenia skrótów,
`handle(WakeInput)`, `addressed(text)`, `mic_state`, `is_listening`).

Wspólny deterministyczny **`WakeMachine`**: PTT (wciśnięcie/puszczenie z `HotkeyPort` — hook `WH_KEYBOARD_LL`;
autopowtórzenia ignorowane), przełącznik, przycisk/Spacja w UI, wyciszenie (blokuje PTT), „nie przeszkadzać”
(nie wyłącza PTT), stan mikrofonu `Off/Listening/Hearing/Processing/Muted`, okno administratora na pierwszym planie
(`blocked_elevated_foreground` raz na epizod), adresowanie z transkryptu (`personas-contract`: imię wygrywa,
inaczej Dyrygentka obsady). `WakeCfg::default()`: PTT `Ctrl+Shift+Space`, przełącznik `Ctrl+Shift+M` (walidacja
reguły AltGr i kill-switcha). Słowa wywoławcze „Hej …” i „zawsze słucham” — tylko typy (`WakeWordCfg`, F5);
konfiguracja z nimi odrzucana. Zdarzenia `voice.wake.*`.
