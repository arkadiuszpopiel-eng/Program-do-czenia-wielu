# voice-cmd — SPEC (szkic v0)

## Cel
Szybka ścieżka komend głosowych bez LLM: „stop", „czekaj", „pauza", „głośniej/ciszej", „wycisz mikrofon", „przełącz na Deltę", „powtórz", „wznów", „anuluj" — z keyword-spotterem (cel < 300 ms od początku słowa) i dopasowaniem z transkryptu partial (PLAN §6.2, §6.5).

## Fala i priorytet
F2. P0.

## Kontrakt (szkic Rust)
```rust
// voice-cmd-contract — SZKIC
pub enum VoiceCommand { Stop, Wait, Pause, Resume, Repeat, Cancel, VolumeUp, VolumeDown, MuteMic, SwitchPersona(PersonaId), SetAutonomy(AutonomyLevel) /* wymaga Brokera */ }
pub struct CmdHit { pub cmd: VoiceCommand, pub source: CmdSource /* Kws | Partial | Final */, pub confidence: f32, pub ts: Instant, pub standalone: bool }
pub trait VoiceCmd: Send + Sync {
    fn push_audio(&self, frame: &Processed) -> Option<CmdHit>;         // KWS „stop/czekaj" na CPU
    fn push_text(&self, t: &Transcript, state: DialogState) -> Option<CmdHit>; // reguły na partial/final
    fn grammar(&self) -> Grammar;                                          // frazy PL/EN, synonimy, edytowalne
}
```
Zdarzenia: `cmd.detected` (komenda, źródło, opóźnienie), `cmd.ignored` (np. „nie" poza `Speaking`), `cmd.forwarded_to_broker` (zmiana autonomii, akcja ryzykowna).

## Zależności
`core-bus/config/log-contract`, `voice-dsp-contract` (ramki), `voice-stt-contract` (partial), `voice-dialog-contract` (stan), `voice-audio-contract` (głośność/mute), `personas-contract` (imiona), `safety-broker-contract` (F3: komendy zmieniające uprawnienia). Zewnętrzne: mały model KWS (spike h, CPU).

## Niezmienniki
- Zero LLM w ścieżce; decyzja z KWS ≤ 300 ms od początku słowa; z partial ≤ 100 ms od transkryptu.
- „nie" jako przerwanie tylko samodzielne, z pauzą przed i po, tylko w `Speaking`; „stop" zawsze działa w `Speaking`/`Thinking`.
- Komendy zmieniające uprawnienia („pracuj na Maksie") nie wykonują się tu — trafiają do Brokera (potwierdzenie fizycznym wejściem).
- Komendy z TV/rozmowy obok nie wyzwalają akcji: wymagany adresat (PTT/wake/imię) lub — w trybie rozmowy — otwarta tura; do `voice-speaker` (F5) bez weryfikacji mówcy komendy tylko nieryzykowne (stop, głośność).
- Gramatyka PL/EN edytowalna (R0), z synonimami; brak akcji destrukcyjnych na liście.

## Zdolności / uprawnienia
Brak; `SetAutonomy` przekazywane do `safety-broker`.

## Izolacja
`inproc`, `lazy`.

## Budżet zasobów
KWS CPU ≤ 3% jednego rdzenia baseline; RAM ≤ 20 MB; reakcja < 300 ms.

## Konfiguracja (klucze TOML)
`[voice.cmd] kws = true`, `kws_words = ["stop", "czekaj"]`, `grammar = "personas/commands.toml"`, `require_addressing = "auto"`.

## Wkład do UI
Ściągawka komend w trybie głosowym; Ustawienia → Głos → Komendy (edytor gramatyki).

## Testy akceptacyjne
- `ACC-F2-voice-cmd-01`: recall „stop/anuluj" ≥ 99% na ≥ 200 próbach z korpusu, reakcja < 300 ms.
- `ACC-F2-voice-cmd-02`: „nie no, dobrze" i podobne (≥ 100 przypadków) → 0 fałszywych przerwań.
- `ACC-F2-voice-cmd-03`: 1 h tła TV → 0 wykonanych komend.

## Fake
`voice-cmd-fake`: trafienia z adnotacji (czas, komenda) — testy `voice-dialog` i UI.

## Otwarte pytania
- Model KWS (własny trening na mowie syntetycznej + korpus) — spike (h); wspólny z `voice-wake` v1.
