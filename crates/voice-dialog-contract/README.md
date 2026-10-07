# voice-dialog-contract

Kontrakt automatu rozmowy głosowej (PLAN §6.5, VOICE.md §5–§10). Automat jest czystą funkcją
`DialogAutomaton::step(&DialogState, &DialogEvent, now_ms) -> Transition { state, commands }` — bez I/O.
Zawiera: fazy `Idle → Listening → UserSpeaking → Thinking → Speaking → Interrupted`, stan (`DialogState`:
tura użytkownika, wypowiedź agentki z fragmentami TTS i znacznikami słów, kandydat na przerwanie, ostatnie
przerwanie, oczekująca wypowiedź), zdarzenia (VAD, transkrypt, koniec tury, komendy `voice-cmd`, tekst z
composera, TTS/odtwarzanie, głośnik, mowa proaktywna, DND, `Esc`, `Tick`), polecenia (`DuckOutput`,
`StopTts`, `CancelGeneration`, `ClearSpeechQueue`, `StartListening`, `SubmitTurn{text, heard_prefix,
interrupted_intent}`, `ResumeFrom{offset}`, `AcquireSpeaker`/`ReleaseSpeaker`, `SpeakProactive`, fillery,
`KillSwitch`, powiadomienia `voice.dialog.*`), `HeardPrefix` (źródło, `approximate`), `InterruptIntent`,
`DialogConfig` (ducking −15 dB, potwierdzenie 200 ms, backchannel), traity `InterruptClassifier`,
`SpeakerLock` (zasób „głośnik”, realny ze `scheduler-lite`), `WordAligner` (forced alignment) oraz helper
`drive` i — pod feature `contract-tests` — `contract_tests::run_all` (niezmienniki dla impl i fake).
