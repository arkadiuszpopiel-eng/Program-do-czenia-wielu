# voice-dialog-fake

Atrapy `voice-dialog` do testów UI, `agent-runtime` i potoku głosu na wirtualnym zegarze:
- `FakeDialog` — uproszczony automat (`DialogAutomaton`): mowa użytkownika w `Speaking` = natychmiastowy twardy stop
  (bez duckingu i backchannelu), prefiks z dokładnością do fragmentu TTS (`approximate`), intencja przerwania
  zawsze „korekta”, bez fillerów i wznawiania; spełnia wspólny test kontraktowy;
- `FakeSpeakerLock` — zasób „głośnik” w pamięci z dziennikiem (`occupy` symuluje inną agentkę/sesję);
  realny zasób przyjdzie ze `scheduler-lite`;
- `ScriptedClassifier` — intencje przerwań z adnotacji (domyślnie korekta);
- `UniformAligner` — alignment słów proporcjonalny do długości (źródło prefiksu nr 2 w testach).

Zależy wyłącznie od crate'ów `*-contract`.
