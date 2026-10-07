# voice-audio-impl

Implementacja `voice-audio` (ADR 0011): **WASAPI przez crate `wasapi` 0.24** (Windows), `UnsupportedAudio` gdzie indziej.

- Tryb współdzielony sterowany zdarzeniami, `f32` z autokonwersją formatu; osobny wątek na strumień
  (`alfa-audio-render` / `alfa-audio-capture`), bufory przygotowane przed pętlą — w pętli brak alokacji
  (test `tests/no_alloc.rs` z licznikiem alokacji `stats_alloc`: 0 alokacji/realokacji/zwolnień na 200 okresów
  renderu + konwersji + zapisu przechwytywania, ACC-F2-voice-audio-04).
- Czas: `IAudioClock::GetPosition` (pozycja + QPC w jednostkach 100 ns) → czas odtworzenia bloku i opóźnienie
  wyjścia (`written − played`); przechwytywanie: znacznik QPC pakietu. Ten sam zegar dla wejścia i wyjścia.
  `GetStreamLatency` nie jest wystawione przez `wasapi` — opóźnienie liczymy z zegara urządzenia.
- Hot-plug: `IMMNotificationClient` (rejestracja żyje na własnym wątku COM), zdarzenia uzupełniane o opis urządzenia.
- Błędy: `AUDCLNT_E_DEVICE_IN_USE` → `ExclusiveConflict`, `E_ACCESSDENIED` → `PermissionDenied`,
  `AUDCLNT_E_DEVICE_INVALIDATED` → strumień zamknięty (`AudioError::Closed`).
- `VoiceAudioModule` (manifest `module.toml`): publikuje zmiany urządzeń i ostrzeżenie Bluetooth na magistralę.

Wymaga Windows/sprzętu (self-hosted): `tests/module.rs::contract_suite_on_hardware` (`#[ignore]`), pomiary
duckingu/`stop_all` loopbackiem, 1 h bez underrunów, MMCSS „Pro Audio” (wymaga `avrt` — do dodania w
`platform-windows-impl`, poza zakresem tego crate'a). Bramka: `cargo clippy -p voice-audio-impl --target
x86_64-pc-windows-msvc --all-targets -- -D warnings`. `deny.toml`: `wasapi` jest wrapperem `windows` 0.62.
