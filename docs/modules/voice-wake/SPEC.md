# voice-wake — SPEC (szkic v0)

## Cel
Aktywacja słuchania i adresowanie: v0 — push-to-talk / przełącznik (hook `WH_KEYBOARD_LL` przez `platform-windows`) + adresowanie po imieniu z transkryptu („Delta, …"); v1 — opcjonalne słowa wywoławcze „Hej Alfa/Beta/Gama/Delta" (własny trening; openWakeWord tylko EN), tryb „zawsze słucham" z bramką właściciela, „nie przeszkadzać" (PLAN §6.2, §7.3, §16.2).

## Fala i priorytet
F2: v0 (PTT/toggle, imię z transkryptu). F5: v1 (wake words po spełnieniu FAR/FRR, bramka `voice-speaker`). P0 (v0).

## Kontrakt (szkic Rust)
```rust
// voice-wake-contract — SZKIC
pub enum WakeMode { PushToTalk { key: Hotkey }, Toggle { key: Hotkey }, WakeWord { phrases: Vec<PersonaId> } /* v1 */, AlwaysOn { owner_gate: bool } /* v1 */ }
pub enum WakeEvent { ListenStart { addressed: Option<PersonaId>, source: WakeSource /* Ptt | Toggle | WakeWord | Name | Ui */ }, ListenStop, AddressedTo(PersonaId), Dnd(bool) }
pub trait Wake: Send + Sync {
    fn set_mode(&self, m: WakeMode) -> Result<()>;
    fn events(&self) -> Subscription<WakeEvent>;
    fn addressed(&self, t: &Transcript) -> Option<PersonaId>;    // „Delta, …” / „Hej Gama”
    fn push_audio(&self, frame: &Processed) -> Option<WakeEvent>; // v1 KWS
}
```
Zdarzenia: `wake.listen_start/stop`, `wake.addressed`, `wake.blocked_elevated_foreground` (okno admina — hook nie działa), `wake.false_alarm_suspected` (v1), `wake.dnd`.

## Zależności
`core-bus/config/log-contract`, `platform-windows-contract` (hook PTT, skróty), `voice-stt-contract` (imiona z transkryptu), `personas-contract` (imiona, frazy), `voice-dialog-contract`, `voice-speaker-contract` (F5), `device-profile-contract`/`shell-integration-contract` (DND).

## Niezmienniki
- PTT zgłasza puszczenie klawisza (hook, nie zwykły skrót); przy oknie administratora na pierwszym planie bez helpera `uiAccess` PTT nie działa — UI komunikuje to wprost.
- Zwrot po imieniu zawsze wygrywa; bez imienia odpowiada agentka z rolą Dyrygentki.
- Wake words włączane dopiero po spełnieniu FAR ≤ 1/dzień i FRR ≤ 5% (F5); domyślnie wyłączone.
- Tryb „zawsze słucham" wymaga bramki właściciela (`voice-speaker`); bez niej niedostępny.
- DND wyłącza wake/proaktywną mowę, nie wyłącza PTT.
- Wskaźnik prywatności w zasobniku przy otwartym mikrofonie.

## Zdolności / uprawnienia
Brak.

## Izolacja
`inproc`, `lazy` (v0 lekki; v1 KWS na CPU z potokiem głosu).

## Budżet zasobów
v0: RAM ≤ 1 MB, reakcja PTT ≤ 10 ms; v1: KWS CPU ≤ 3% rdzenia, RAM ≤ 30 MB.

## Konfiguracja (klucze TOML)
`[voice.wake] mode = "ptt"`, `ptt_key = "Space"` (poza polem tekstowym), `toggle_key = "Ctrl+Shift+M"`, `name_addressing = true`; v1: `wake_words = false`, `phrases = ["Hej Alfa", "Hej Beta", "Hej Gama", "Hej Delta"]`, `always_on = false`.

## Wkład do UI
Przycisk mikrofonu i stany (wyłączony/słucha/…), przytrzymanie Spacji = mów, `Ctrl+Shift+M`, Ustawienia → Głos → Słowa wywoławcze, komunikat „okno administratora na wierzchu".

## Testy akceptacyjne
- `ACC-F2-voice-wake-01`: PTT start/stop ≤ 10 ms od klawisza (hook), 100/100 na runnerze; toggle działa z ukrytym oknem.
- `ACC-F2-voice-wake-02`: adresowanie po imieniu z transkryptu ≥ 98% na zestawie ≥ 100 wypowiedzi z korpusu.
- `ACC-F5-voice-wake-03`: FAR ≤ 1/dzień na ≥ 24 h tła PL (TV, podcasty); FRR ≤ 5% na ≥ 200 pozytywach właściciela.

## Fake
`voice-wake-fake`: zdarzenia PTT/adresowania ze skryptu na wirtualnym zegarze.

## Otwarte pytania
- Trening KWS „Hej …" (dane syntetyczne + korpus) — spike (h); frazy 3–4-sylabowe (ryzyko krótkich imion).
