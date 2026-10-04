# voice-speaker — SPEC (v1, F5: kontrakt, fake i impl w repo)

## Cel
Weryfikacja właściciela głosem (PLAN §6.2, §6.10, §8.3; VOICE.md §13): rejestracja (≥ 3 wypowiedzi), embedding mówcy (ECAPA/WeSpeaker przez ONNX), weryfikacja z progiem i pewnością, profil **zaszyfrowany lokalnie**, usuwanie, eksport tylko za jawną zgodą. Wynik zasila istniejące pole `CommandOrigin::UserVoice { confidence, speaker_verified }` — akcja ryzykowna z głosu bez weryfikacji wymaga potwierdzenia nie-głosem.

## Fala i priorytet
F5, P1 (warunek F5-07/08; bramka właściciela dla `voice-wake` v1).

## Kontrakt (Rust, `voice-speaker-contract`)
```rust
pub trait SpeakerVerifier: Send + Sync {
    fn status(&self) -> EnrollmentStatus;                 // NotEnrolled | Enrolling { done, needed, has_profile } | Enrolled { utterances, model }
    fn begin_enrollment(&self) -> Result<(), SpeakerError>;
    fn add_enrollment(&self, audio: &[f32]) -> Result<EnrollProgress, SpeakerError>;   // 16 kHz mono; długość ≥ min_enroll_ms, poziom ≥ min_level_db
    fn finish_enrollment(&self) -> Result<EnrollmentStatus, SpeakerError>;             // ≥ 3 spójne → profil = znormalizowana średnia
    fn cancel_enrollment(&self);                          // embeddingi zerowane
    fn verify(&self, audio: &[f32]) -> Result<Verification, SpeakerError>;             // { score, confidence, decision: Rejected | Likely | Verified, audio_ms, model }
    fn delete(&self) -> Result<bool, SpeakerError>;       // crypto-shredding
    fn export(&self, consent: Option<&ExportConsent>) -> Result<SpeakerExport, SpeakerError>;   // None → ConsentRequired
    fn config(&self) -> SpeakerCfg;  fn take_events(&self) -> Vec<SpeakerEvent>;
}
pub trait EmbeddingModel: Send { fn model_id(&self) -> &str; fn embed(&mut self, audio: &[f32]) -> Result<Embedding, SpeakerError>; }
pub trait ProfileStore: Send + Sync { fn load(&self) -> …Option<Profile>; fn save(&self, &Profile); fn delete(&self) -> …bool; }
pub struct SpeakerEngine<M: EmbeddingModel>;              // rdzeń bez I/O, wspólny dla -impl i -fake
pub enum SpeakerCheck { NotChecked, Pending, Checked { decision, score_permille } }   // verified() ⇔ decision == Verified
pub fn voice_origin(stt_confidence: f32, speaker: &SpeakerCheck) -> CommandOrigin;   // speaker_verified tylko przy progu ścisłym
pub mod eer { rates_at, eer, threshold_for_far, report }  // EER_MAX 3 %, FAR_STRICT_MAX 0,1 %, MIN_IMPOSTOR_TRIALS 3000
```
Zdarzenia (bez audio i embeddingu): `voice.speaker.enroll_started`, `enroll_sample` (przyjęta/odrzucona + powód), `enrolled`, `verified` (wynik ‰, decyzja), `deleted`, `export` (z/bez zgody).

## Zależności
`core-bus-contract`, `voice-dsp-contract` (fbank), `sessions-contract` (`KeyVault` — klucz profilu), `risk-classifier-contract` (`RiskLevel`, `CommandOrigin`).

## Niezmienniki
- Profil wyłącznie lokalnie: plik `ALFASPK1 ‖ nonce ‖ XChaCha20-Poly1305(JSON)`, klucz 32 B w Windows Credential Manager (`alfa/voice/speaker`), zapis atomowy, bufory zerowane (`zeroize`); poza katalogami sesji — nie trafia do eksportu `.alfa`.
- Eksport tylko z `ExportConsent` (jawna, ze znacznikiem czasu i celem); bez zgody → `ConsentRequired` + zdarzenie odmowy.
- Usunięcie kasuje plik i klucz (crypto-shredding); po nim `verify` → `NotEnrolled`.
- `verify` przed rejestracją / za krótka wypowiedź → błąd ⇒ tura **niezweryfikowana** (fail-closed). `speaker_verified = true` tylko przy progu ścisłym.
- Destrukcja głosem zawsze wymaga potwierdzenia nie-głosem (reguła `VoiceDestructive` Jądra) — weryfikacja nie chroni przed odtworzeniem nagrania (anti-spoofing poza v1).
- Model ładowany tylko po zgodności SHA-256 z manifestem.

## Zdolności / uprawnienia
Brak (dostęp do Credential Manager przez `KeyVault` aplikacji).

## Izolacja
`inproc`, `lazy` (model ładowany przy pierwszej weryfikacji/rejestracji).

## Budżet zasobów
RAM ≤ 80 MB (model ~20–30 MB), CPU śr. ≤ 3 % (embedding raz na wypowiedź, ≤ 100 ms dla 3 s audio na baseline — do pomiaru).

## Konfiguracja (klucze TOML)
`[voice.speaker] model = "<ścieżka>.speaker.json"`, `threshold_standard = 0.45`, `threshold_strict = 0.62` (startowe — ustala `alfa-speaker-eval` na korpusie), `min_enroll_ms = 1500`, `min_verify_ms = 800`, `max_enroll_utterances = 12`, `min_consistency = 0.3`, `min_level_db = -50`.

## Wkład do UI
Ustawienia → Głos → „Rozpoznawanie mojego głosu”: rejestracja (≥ 3 zdania, postęp i powód odrzucenia), stan, usuń profil, eksport (dialog zgody). Karta Brokera: „Głos niezweryfikowany — potwierdź kliknięciem”.

## Testy akceptacyjne
- `ACC-F5-voice-speaker-01` (CI): kontraktowe na atrapie i impl (rejestracja/odrzucenia, weryfikacja właściciel/obcy, eksport bez zgody → odmowa, usunięcie = shredding, zaszyfrowany plik bez jawnego embeddingu), syntetyczny model ONNX (tract) rozróżnia mówców, EER runner na syntetycznych embeddingach.
- `ACC-F5-voice-speaker-02` (self-hosted, F5-07/08): EER ≤ 3 %, FAR ≤ 0,1 % przy progu ścisłym na ≥ 3000 próbach obcych (Common Voice PL + TTS) — `alfa-speaker-eval run`.
- `ACC-F5-voice-speaker-03` (CI): potok — tura głosowa bez weryfikacji ⇒ `speaker_verified = false`, z weryfikacją ścisłą ⇒ `true` (`voice-pipeline-impl/tests/e2e_speaker.rs`).

## Fake
`voice-speaker-fake`: `FakeSpeaker` = `SpeakerEngine<PitchEmbedder>` (deterministyczny miękki histogram tonu podstawowego) + `MemoryProfileStore`, głosy syntetyczne `owner_voice`/`stranger_voice`; przechodzi testy kontraktowe.

## Otwarte pytania
- Kalibracja pewności (dziś logistyka wokół progów) — dopasowanie z runnera EER na korpusie.
- Anti-spoofing (odtworzenie nagrania) — poza v1.

## Decyzje v1 (F5)
- Dwa progi: standardowy (≈ EER) → `Likely`, ścisły (FAR ≤ 0,1 %) → `Verified`; `SpeakerCfg::threshold_for(RiskLevel)`: niskie — standardowy, średnie+ — ścisły.
- Model: manifest `alfa-speaker-v1` (`<model>.speaker.json`: ONNX + SHA-256, `n_mels` 80, okno `frames` × 10 ms, `hop_frames`, CMN), cechy Kaldi fbank w Rust, embedding = średnia znormalizowanych okien; inferencja `tract-onnx` 0.23.8. Modelu nie ma w repo (README `voice-speaker-impl`: źródła, licencje).
- `SpeakerOwnerCheck` (impl) = bramka właściciela słów wywoławczych (próg standardowy; brak profilu → `None` → odrzucenie).
- Runner `alfa-speaker-eval` rejestruje w profilu tymczasowym w pamięci (nigdy w profilu użytkownika); format `evals/F5/voice/README.md` §2.

## Implementacja w aplikacji (F5, `app-voice`)
- Komenda `voice_speaker`: kreator `begin` → `record_start`/`record_stop` (nagranie z mikrofonu z dzierżawą `scheduler-lite`, resampling do 16 kHz, maks. 8 s, bufor zerowany po użyciu) → wskaźnik jakości (`good`/`too_short`/`too_quiet`/`inconsistent`, poziom dBFS, długość) → `finish` (≥ 3 frazy); `cancel`, `delete` (crypto-shredding), `set_required`.
- Produkcja: `open_speaker(<modele>/speaker/*.speaker.json, <local>/voice/speaker.profile, StoreKeyVault(Credential Manager))` — profil poza katalogami sesji (nie trafia do `.alfa`), model ładowany leniwie (pierwszy widok/rejestracja/weryfikacja).
- Potok dostaje `GatedVerifier`: przy wyłączonym „wymagaj weryfikacji dla akcji ryzykownych” (`voice.speaker_required`, domyślnie wł.) audio nie trafia do modelu. `ChatReply` czeka na wynik ≤ 1,5 s; `VoiceTurnOrigin { stt_confidence_permille, speaker_verified }` → `CommandOrigin::UserVoice` w `app-core`. Głos niezweryfikowany ma pewność STT ograniczoną do 700‰ (< 800‰), więc każda zmiana stanu zlecona nim pyta nie-głosem; zweryfikowany właściciel — reguły klasyfikatora wg poziomu autonomii. Reguły Jądra bez zmian (destrukcja głosem zawsze nie-głosem).
- Testy: `crates/app-voice/tests/f5_speaker.rs` (kreator, obcy głos przy akcji ryzykownej → `Ask { non_voice }` na L4, właściciel → `Proceed`, destrukcja → `Ask { non_voice }`, wyłączony przełącznik → niezweryfikowany).
