# voice-speaker-impl

Implementacja `voice-speaker` (docs/modules/voice-speaker/SPEC.md): rdzeń `SpeakerEngine` z kontraktu +

- **`OnnxSpeakerModel`** — embedding mówcy z modelu ONNX przez **`tract-onnx` 0.23.8** (czysty Rust, jak
  `voice-vad-impl`): manifest `alfa-speaker-v1` (`<model>.speaker.json`: plik ONNX z **SHA-256** — inny plik nie jest
  ładowany, `n_mels` (80), okno `frames` ramek 10 ms ze stałym kształtem `[1, frames, n_mels]`, `hop_frames`, CMN).
  Cechy Kaldi fbank liczone w Rust (`voice-dsp-contract::fbank`), embedding = średnia znormalizowanych okien.
- **`EncryptedFileStore`** — profil w pliku `ALFASPK1 ‖ nonce ‖ XChaCha20-Poly1305(JSON)` (AAD = nagłówek), klucz 32 B
  z `sessions-contract::KeyVault` pod nazwą `alfa/voice/speaker` (w aplikacji: `StoreKeyVault` z `app-modules` —
  Windows Credential Manager); zapis atomowy; **usunięcie kasuje plik i klucz (crypto-shredding)**; bufory zerowane.
  Profil leży poza katalogami sesji — nie trafia do eksportu `.alfa`; eksport tylko przez `export(Some(&ExportConsent))`.
- **`SpeakerOwnerCheck`** — bramka właściciela dla słów wywoławczych (`voice-wake` v1, `owner_gate`; fail-closed).
- **`eval`** + bin **`alfa-speaker-eval`** (`check`, `run`, `schema`) — EER, FAR/FRR przy progach, próg dla FAR ≤ 0,1%;
  rejestracja w profilu tymczasowym w pamięci (nigdy w profilu użytkownika). Format: `evals/F5/voice/README.md` §2.

## Skąd wziąć model (nie ma go w repo)

| Model | Skąd | Licencja | Uwagi |
|---|---|---|---|
| WeSpeaker ResNet34 / ECAPA (VoxCeleb) | sherpa-onnx „speaker-recongition-models” (wydania GitHub `k2-fsa/sherpa-onnx`, np. `wespeaker_en_voxceleb_resnet34.onnx`) albo eksport z repozytorium WeSpeaker | Apache-2.0 (kod); modele trenowane na VoxCeleb — **sprawdzić warunki danych przed dystrybucją** | wejście fbank 80 + CMN, wyjście 256 |
| 3D-Speaker ERes2Net / CAM++ (zh/en) | sherpa-onnx (jw.) / ModelScope | Apache-2.0 | wejście fbank 80 |
| SpeechBrain ECAPA-TDNN (VoxCeleb) | `speechbrain/spkrec-ecapa-voxceleb` (eksport do ONNX własnym skryptem) | Apache-2.0 | wejście fbank 80, wyjście 192 |

Z tej sesji GitHub i HuggingFace są zablokowane, więc model trzeba pobrać na maszynie użytkownika, policzyć SHA-256
(`sha256sum`) i wpisać do manifestu. Progi (`threshold_standard` ≈ punkt EER, `threshold_strict` = FAR ≤ 0,1%) ustala
runner na korpusie (bramka #3: typ 11 + Common Voice PL + TTS; ≥ 3000 prób obcych dla F5-08):
`cargo run -p voice-speaker-impl --bin alfa-speaker-eval -- run evals/corpus/f5/speaker.ndjson --audio-root evals/corpus
--model …/x.speaker.json --out próby.ndjson`. Test z prawdziwym modelem: `ALFA_SPEAKER_MODEL=… ALFA_SPEAKER_WAVS=a,b,c,d
cargo test -p voice-speaker-impl --test speaker real_model -- --ignored --nocapture`.

Nie chroni przed odtworzeniem nagrania właściciela (anti-spoofing poza v1) — dlatego destrukcja głosem zawsze wymaga
potwierdzenia nie-głosem (reguła Jądra), a weryfikacja tylko podnosi `speaker_verified` dla reguły `VoiceUnverifiedRisky`.
