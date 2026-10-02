# voice-s2s-fake

Atrapa trybu speech-to-speech (docs/modules/voice-s2s/SPEC.md): `FakeS2sClient` waliduje konfigurację jak kontrakt
(sesja `Private` nie łączy się — licznik połączeń zostaje 0), odpowiada na zatwierdzoną turę skryptem
(`script_reply`), strumieniuje audio odpowiedzi po 100 ms na `poll`, sygnalizuje `UserSpeechStarted` przy głośnym
audio w trakcie odpowiedzi (VAD serwera), a `truncate` obcina tekst odpowiedzi w historii dostawcy proporcjonalnie do
usłyszanego audio — bez notki (`NativeTruncate`). Przechodzi testy kontraktowe (`tests/fake.rs`).
