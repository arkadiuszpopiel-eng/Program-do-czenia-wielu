# voice-dictation-contract

Kontrakt dyktowania (docs/modules/voice-dictation/SPEC.md): trait `Dictation` (`start(mode)`, `on_final`, `tick`,
`undo_last`, `stop`), rdzeń `DictationMachine` (cel = okno z chwili startu, pauza przy zmianie okna, kolejka fraz,
historia do „cofnij to”, blokada Enter w terminalach), `normalize` (komendy interpunkcji PL, „dosłownie X”,
interpunkcja STT zastępowana komendą, odstępy i wielkie litery), `numbers` (liczebniki → cyfry tylko jednoznacznie),
`control_command` („cofnij to”, „koniec dyktowania” — tylko cała wypowiedź). Zdarzenia `voice.dictation.*` bez treści.
Przypadki normalizacji: `evals/F5/voice/dictation-cases.json` (F5-10, część CI).
