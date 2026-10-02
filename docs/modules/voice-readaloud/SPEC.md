# voice-readaloud — SPEC (v1, F5: kontrakt, fake i impl w repo)

## Cel
Czytanie na głos zaznaczenia albo tekstu okna na pierwszym planie (PLAN §6.2, §16.2 F5; ACCEPTANCE F5-11): odczyt UIA `TextPattern` (tylko do odczytu; zapas Ctrl+C z przywróceniem schowka), segmentacja na zdania, synteza głosem wybranej agentki, sterowanie (pauza, dalej, wstecz, szybciej/wolniej). Treść okna jest **niezaufana** — nie trafia do pamięci ani modelu bez zgody. Nigdy pola haseł.

## Fala i priorytet
F5, P1 (po `platform-windows` v1.5: UIA `TextPattern`).

## Kontrakt (Rust, `voice-readaloud-contract`)
```rust
#[async_trait] pub trait ReadAloud: Send {
    async fn start(&mut self, scope: ReadScope /* Selection | Document */, persona: PersonaId) -> Result<ReadStatus, ReadAloudError>;
    async fn step(&mut self) -> ReadStatus;                  // ~20 ms: synteza, odtwarzanie, następne zdanie
    async fn control(&mut self, c: ReadControl) -> ReadStatus;   // Pause | Resume | Next | Previous | Faster | Slower | Restart | Stop
    fn status(&self) -> ReadStatus;                          // faza, zdanie i liczba zdań, podświetlenie (offsety), tempo, aplikacja
    fn share_with_model(&self, consent: Option<&ShareConsent>) -> Result<String, ReadAloudError>;   // tylko za zgodą, blok niezaufany
    fn take_events(&mut self) -> Vec<ReadAloudEvent>;
}
pub trait TextSource: Send + Sync { fn read(&self, scope: ReadScope, max_chars: usize) -> Result<SourceText, ReadAloudError>; }
pub trait SelectionReader: Send + Sync { fn selection(&self, w: WindowId, max_chars: usize) -> Result<Option<String>, String>; }   // port UIA (do platform-contract)
pub fn segment(text: &str, max_chars: usize) -> Vec<Segment>;   // zdania PL z offsetami
pub struct ReadAloudMachine;    // rdzeń sterowania bez I/O, wspólny dla -impl i -fake
pub struct UntrustedText;       // bez Display; for_model(Some(zgoda)) → wrap_untrusted
pub enum RefuseReason { NoForeground, ProtectedTarget, PasswordField, NoText, Unsupported }
```
Zdarzenia (bez treści — liczby zdań/znaków, aplikacja): `voice.readaloud.started`, `segment`, `paused`, `resumed`, `rate`, `finished`, `stopped`, `refused`, `failed`.

## Zależności
`core-bus-contract`, `platform-contract` (`DesktopPort`, `UiaPort`, `InputPort`, `ClipboardPort`, `TargetGuard`), `personas-contract`, `tools-common-contract` (`wrap_untrusted`), `voice-tts-contract`, `voice-audio-contract`, `providers-contract` (`PrivacyTag`), `scheduler-lite-contract` (głośnik).

## Niezmienniki
- Odczyt wyłącznie: UIA `TextPattern` bez modyfikacji dokumentu; port nie oddaje pól haseł; fokus w polu hasła → odmowa; okna Alfy/Brokera/nieznanych procesów → odmowa (`TargetGuard`).
- Zapas Ctrl+C tylko gdy pole hasła jest wykluczone (fokus znany i nie-hasło albo w oknie brak pól haseł; błąd UIA = odmowa). Schowek zapamiętany i **wyczyszczony** przed kopiowaniem (brak starej zawartości jako „zaznaczenia”), **zawsze przywracany**.
- Treść niezaufana: nie w zdarzeniach, logach, pamięci; do modelu tylko `share_with_model(Some(ShareConsent))` jako blok danych `<<<NIEZAUFANE` (nie polecenia). `Stop` usuwa treść z pamięci procesu.
- TTS z `PrivacyTag::Private` (treść okna nie idzie do chmurowego TTS), chyba że `allow_cloud_tts = true`.
- Głośnik jako zasób `scheduler-lite` (`Holder::Persona`, `Priority::Interactive`): zajęty → odmowa, odebrany (mowa użytkownika, kill-switch) → stop.

## Zdolności / uprawnienia
Odczyt UIA; `input.synthesize` tylko Ctrl+C do okna celu (zapas); schowek (zapis/przywrócenie).

## Izolacja
`inproc`, `lazy`.

## Budżet zasobów
RAM ≤ 4 MB (tekst ≤ 50 000 znaków), CPU ≤ 1 % (syntezę liczy `voice-tts`); odczyt UIA ≤ 5 s (limit portu).

## Konfiguracja (klucze TOML)
`[voice.readaloud] rate = 1.0`, `rate_step = 0.1`, `min_rate = 0.5`, `max_rate = 2.0`, `max_segment_chars = 300`, `max_chars = 50000`, `clipboard_fallback = true`, `allow_cloud_tts = false`.

## Wkład do UI
Pasek czytania (zdanie n/m, podświetlenie, ⏯, ⏭, ⏮, tempo), skrót „czytaj zaznaczenie”, komunikaty odmowy, „Wyślij agentce” z dialogiem zgody.

## Testy akceptacyjne
- `ACC-F5-voice-readaloud-01` (CI): segmentacja PL (skróty, liczby, daty, inicjały, długie zdania), automat sterowania, kontraktowe na atrapie i impl (`platform-fake` + `voice-tts-fake` + `voice-audio-fake`, wirtualny zegar): dokument, zaznaczenie UIA i Ctrl+C (schowek przywrócony), odmowa dla haseł/okien chronionych, treść tylko za zgodą.
- `ACC-F5-voice-readaloud-02` (self-hosted, F5-11): zaznaczenie czytane w 5 aplikacjach (Notatnik, WordPad, przeglądarka, VS Code, Word).

## Fake
`voice-readaloud-fake`: `FakeReadAloud` — `ReadAloudMachine` na `FakeReadWorld` (okna, zaznaczenia, pola haseł, zegar odtwarzania: czas zdania z długości tekstu i tempa); przechodzi testy kontraktowe.

## Otwarte pytania
- `UiaPort::read_selection` (TextPattern `GetSelection`) w `platform-contract` — dziś `SelectionReader` w kontrakcie modułu, adapter w aplikacji.
- Podświetlanie w oknie źródłowym (UIA `Select`) — poza v1 (modyfikuje stan aplikacji).

## Decyzje v1 (F5)
- Domyślnie lokalny TTS (prywatność), tempo 0,5–2,0 krokiem 0,1, zdania > 300 znaków dzielone na przecinkach/spacjach.
- Bez UIA zaznaczenia: zapas Ctrl+C (włączony domyślnie) z czyszczeniem i przywróceniem schowka.
