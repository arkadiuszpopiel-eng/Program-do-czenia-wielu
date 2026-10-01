# voice-wake-impl

Implementacja `voice-wake` v0: `WakeService` na `HotkeyPort` (`platform-windows-impl`: `RegisterHotKey` +
`WH_KEYBOARD_LL` zgłaszający puszczenie PTT) — rejestracja/wyrejestrowanie skrótów (niepoprawna konfiguracja nie
psuje poprzedniej), `ProcessPort::foreground_is_elevated` przy `pump` (okno administratora → komunikat w UI),
`MicArbiter` — mikrofon jako zasób wyłączny `scheduler-lite` (`Holder::User`, `Priority::UserSpeech`) na czas
słuchania, `VoiceWakeModule` publikuje `voice.wake.*`. Reakcja PTT ≤ 10 ms (ACC-F2-voice-wake-01) — pomiar na runnerze.
