# voice-cmd — SPEC (v1: kontrakt, fake i impl w repo; F2)

## Cel
Szybka ścieżka komend głosowych bez LLM: „stop", „czekaj", „pauza", „głośniej/ciszej", „wycisz mikrofon", „przełącz na Deltę", „powtórz", „wznów", „anuluj" — z keyword-spotterem (cel < 300 ms od początku słowa) i dopasowaniem z transkryptu partial (PLAN §6.2, §6.5).

## Fala i priorytet
F2. P0.

## Kontrakt (v1, `crates/voice-cmd-contract`)
```rust
pub enum VoiceCommand { Stop, Wait, Pause, Resume, Repeat, Cancel, VolumeUp, VolumeDown, MuteMic,
    SwitchPersona { persona: PersonaId }, DoNotDisturb, StopAll /* kill-switch: zatrzymuje → natychmiast */, No /* samodzielne „nie” */ }
pub enum AgentActivity { Silent, Thinking, Speaking }           // z DialogPhase::agent_activity() — bez cyklu voice-cmd ↔ voice-dialog
pub struct Token { text, start_ms, end_ms, confidence }
pub struct CmdInput { tokens, source: Kws|Partial|Final, activity, now_ms, prev_speech_end_ms, addressed }
pub enum CmdDecision { Hit(CmdHit { command, source, confidence, at_ms, standalone, addressed }),
    Pending { recheck_at_ms }, Ignored { command, reason: NieOutsideSpeaking|NotAddressed|LowConfidence }, NoMatch }
pub struct Grammar { rules: Vec<GrammarRule { command, phrases }>, fillers, personas: Vec<PersonaForms>, nie: NieRule, threshold /*0.6*/, settle_ms /*80*/ }
pub trait CommandRecognizer { fn recognize(&self, input: &CmdInput) -> CmdDecision; fn grammar(&self) -> Grammar; }
```
Składnia frazy: `a|b` alternatywy, `[a]` opcjonalne, `{persona}` slot (formy imion w przypadkach). Wypowiedź jest komendą tylko, gdy składa się wyłącznie z fraz komend, wypełniaczy i imion (adresowanie) — każde inne słowo = zwykła wypowiedź (LLM). Tolerancja ASR: `fold` (bez polskich znaków, interpunkcji) + odległość edycyjna (1 dla słów ≥ 5 znaków, 2 dla ≥ 8). Partial → trafienie po `settle_ms` ciszy po ostatnim słowie. KWS na audio (`push_audio`) przyjdzie z modelem (spike h) jako osobna implementacja traitu.
Zdarzenia: `voice.cmd.detected`, `voice.cmd.ignored`, `voice.cmd.forwarded_to_broker`.

## Zależności
v1: `core-bus-contract`, `voice-persona-contract` (`PersonaId`). Stan dialogu przez `AgentActivity` (nie `voice-dialog-contract` — unikamy cyklu). Później: `voice-dsp/stt/audio-contract`, `safety-broker-contract` (F3: `SetAutonomy` → Broker). Zewnętrzne: mały model KWS (spike h, CPU).

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
`voice-cmd-fake`: trafienia z adnotacji (`script`) + dokładne frazy gramatyki (bez tolerancji literówek), ta sama reguła „nie” i adresowanie; przechodzi test kontraktowy.

## Stan testów (v1)
Zestaw deweloperski zamrożony w `crates/voice-cmd-impl/tests/data/` (113 pozytywów, 88 negatywów; SHA-256 + FNV-1a): recall 100 %, odrzucenie negatywów 100 %. Oficjalny zestaw ACC (≥ 200 prób z korpusu) tworzy recenzent w `evals/`.

## Otwarte pytania
- Model KWS (własny trening na mowie syntetycznej + korpus) — spike (h); wspólny z `voice-wake` v1.
