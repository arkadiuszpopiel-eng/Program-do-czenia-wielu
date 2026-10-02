# voice-speaker-fake

Atrapa `voice-speaker`: ten sam rdzeń `SpeakerEngine` co `-impl`, ale z deterministycznym modelem cech
`PitchEmbedder` (miękki histogram tonu podstawowego w 32 przedziałach log 70–400 Hz — głosy syntetyczne o różnym F0
są rozróżnialne) i profilem w pamięci (`MemoryProfileStore`). `owner_voice` / `stranger_voice` — wypowiedzi
syntetyczne (`voice-audio-contract::synth`) do testów kontraktowych i EER w CI (F5-07 na syntetycznych embeddingach).
