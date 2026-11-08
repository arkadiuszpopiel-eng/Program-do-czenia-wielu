# voice-speaker-contract

Kontrakt weryfikacji właściciela (docs/modules/voice-speaker/SPEC.md): trait `SpeakerVerifier` (rejestracja ≥ 3
wypowiedzi, weryfikacja, usuwanie, eksport tylko za `ExportConsent`), `SpeakerCfg` (progi standardowy/ścisły,
`threshold_for(RiskLevel)`, pewność logistyczna), `Embedding` + trait `EmbeddingModel`, `eer` (EER, FAR/FRR, próg dla
FAR ≤ 0,1%, progi F5-07/08), `SpeakerCheck` + `voice_origin` → `CommandOrigin::UserVoice { confidence,
speaker_verified }` (istniejące pola `risk-classifier-contract`; `speaker_verified` tylko przy progu ścisłym).
Zdarzenia `voice.speaker.*` bez audio i embeddingu.
