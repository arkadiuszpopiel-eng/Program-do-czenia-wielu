# app-voice

Tryb głosowy w aplikacji (kategoria `app-*`, korzeń kompozycji wydzielony z `app-core`): `PipelineVoice`
(implementacja `VoicePort`) składa potok `voice-pipeline-impl` z modułów `voice-*-impl` (`SystemVoice`) albo
z fabryki testowej (`VoiceEngineFactory`), odpowiedzi z czatu sesji (`ChatReply`: historia append-only,
usłyszany prefiks), pigułkę i stan trybu głosowego. Komendy `voice_*` (COMMANDS.md) deleguje `app-core`.

## Głos rozszerzony F5
Komendy `voice_features`, `voice_wake`, `voice_speaker`, `voice_dictation`, `voice_read`, zdarzenie
`VoiceFeaturesChanged` (bez audio, embeddingu i czytanego tekstu). Składniki daje fabryka
(`VoiceEngineFactory::features()` → `FeatureFactory`), porty pulpitu i ustawienia — `FeatureDeps`.

- **Słowa wywoławcze** (`features/wake.rs`): domyślnie wyłączone, włączenie tylko jawne; bez pomiaru FAR/FRR
  (`<modele>/kws/calibration.json` = wynik `alfa-wake-eval run`) — tylko z „na własne ryzyko”; bramka właściciela
  (domyślnie wł., fail-closed bez profilu); DND/wyciszenie wstrzymują nasłuch; test wykrycia bez rozmowy.
- **Weryfikacja właściciela** (`features/speaker.rs`): kreator (nagrania z dzierżawą mikrofonu, wskaźnik jakości,
  ≥ 3 frazy), profil szyfrowany w `<local>/voice/speaker.profile` z kluczem w Credential Manager, usuwanie
  (crypto-shredding), przełącznik „wymagaj weryfikacji dla akcji ryzykownych”. Wynik → `VoiceTurnOrigin` →
  `CommandOrigin::UserVoice` (niezweryfikowany głos: pewność STT ≤ 700‰ → zmiany stanu pytają nie-głosem).
- **Dyktowanie** (`features/dictation.rs`): `DictationRunner` + `DictationService` na portach computer use
  (strażnik okien Alfy/Brokera, odmowa w polach haseł), profile aplikacji, podgląd ostatniej frazy tylko w UI,
  skrót `Ctrl+Alt+D` (powłoka).
- **Czytanie** (`features/read.rs`): zaznaczenie/dokument (UIA, zapas Ctrl+C) albo schowek głosem bieżącej
  agentki, kolejka, tempo, stop (`Esc`, „stop”, kill-switch), skrót `Ctrl+Alt+R`.
- W trakcie dyktowania i czytania tury głosowe nie trafiają do modelu (`reply.rs`); S2S — tylko stan „wymaga klucza”.
- Ustawienia: pomocnicze klucze `core-config` `voice.wake_words`, `voice.wake_accept_risk`,
  `voice.wake_owner_gate`, `voice.speaker_required`, `voice.read_rate`, `voice.dictation_profiles`.

Testy (`tests/f5_*.rs`): potok `voice-pipeline-impl` na atrapach `voice-*-fake`, `platform-fake` i wirtualnym
zegarze `FakeAudio` — wake → tura → odpowiedź, obcy mówca przy akcji ryzykownej (werdykt `risk-classifier`),
dyktowanie do pola hasła = odmowa, czytanie przerwane „stop”.
