# model-residency — SPEC (szkic v0)

## Cel
Zarządca rezydencji modeli w RAM/VRAM: limity per maszyna, dzierżawy (lease) dla STT/TTS/LLM/embeddera/VAD, kolejność wymiany (STT+TTS+LLM nie zawsze naraz), zwalnianie po bezczynności, brak wyścigu o VRAM z grami (pełny ekran → STT/LLM na CPU lub chmurę), tryb baterii (PLAN §3.4, §3.5, §6.3).

## Fala i priorytet
F2. P0. (W F1 `providers-local` używa prostego limitu; migracja do lease w F2.)

## Kontrakt (szkic Rust)
```rust
// model-residency-contract — SZKIC
pub struct Budget { pub vram_mb: u32, pub ram_mb: u32, pub reserved_desktop_vram_mb: u32 /* 512–1024 */ }
pub struct LeaseRequest { pub owner: ModuleId, pub model: ModelId, pub vram_mb: u32, pub ram_mb: u32, pub priority: Priority /* VoiceRt > Conversation > Background */,
                          pub placement: Placement /* GpuPreferred | CpuOnly | Any */, pub idle_unload: Duration }
pub struct Lease { pub id: LeaseId, pub placement: Placement, pub granted: Timestamp }
pub trait Residency: Send + Sync {
    fn acquire(&self, r: LeaseRequest) -> Result<Lease, ResidencyError /* Wait { eta } | Evictable { victims } | Denied */>;
    fn release(&self, id: LeaseId);
    fn touch(&self, id: LeaseId);                          // odświeża licznik bezczynności
    fn snapshot(&self) -> ResidencyState;                  // co jest gdzie, ile wolne
    fn set_mode(&self, m: Mode /* Normal | Gaming | Battery | EmulatedBaseline(Budget) */);
}
```
Zdarzenia: `residency.granted/released/evicted` (kto, dlaczego), `residency.mode_changed`, `residency.oom_avoided`, `residency.budget_exceeded`.

## Zależności
`core-bus/config/log-contract`, `device-profile-contract` (VRAM, bateria, pełny ekran), `platform-windows-contract` (pomiar użycia VRAM/RAM). Klienci: `voice-stt/tts/vad/turn`, `providers-local`, `search` (embedder).

## Niezmienniki
- Suma dzierżaw ≤ budżet maszyny minus rezerwa pulpitu; przekroczenie = `Wait`/`Evictable`, nigdy cichy OOM.
- Eksmisja wg priorytetu i bezczynności: nigdy nie eksmituje `VoiceRt` w trakcie tury na rzecz `Background`.
- Tryb `Gaming` (pełny ekran): nowe dzierżawy GPU odrzucane, istniejące STT/LLM przenoszone na CPU/chmurę przy najbliższej okazji.
- Tryb `Battery`: modele tła nie ładowane; konsolidacja/Ulepszacz wstrzymane.
- Laptop 6 GB: STT i ciężki TTS nie rezydentne naraz (wymiana); baseline 8 GB: pulpit 0,5–1 GB + STT 1–2,5 GB + LLM 3–4B.
- Emulacja baseline dostępna w buildzie testowym (limity dla CI sprzętowego).

## Zdolności / uprawnienia
Brak.

## Izolacja
`inproc`, `always` (lekki; decyzje synchroniczne).

## Budżet zasobów
RAM ≤ 1 MB; `acquire` ≤ 1 ms (bez ładowania — ładuje klient po przyznaniu).

## Konfiguracja (klucze TOML)
Per maszyna: `[machine.residency] vram_mb = "auto"`, `ram_mb = "auto"`, `desktop_reserve_mb = 768`, `gaming_mode = "auto"`, `battery_mode = "auto"`, `idle_unload_default = "10m"`; wspólne: `[residency] priority_order = ["voice_rt", "conversation", "background"]`.

## Wkład do UI
Ustawienia → Urządzenia/Moduły (co jest załadowane, ile VRAM), stan „GPU OOM" (§14.4), Zdrowie systemu (F8).

## Testy akceptacyjne
- `ACC-F2-model-residency-01`: property-based — dowolna sekwencja acquire/release nie przekracza budżetu; brak zakleszczeń (1000 losowych scenariuszy).
- `ACC-F2-model-residency-02`: laptop (6 GB): STT + TTS ciężki → wymiana bez crashu, rozmowa trwa (runner laptop).
- `ACC-F2-model-residency-03`: pełny ekran (gra) → 0 nowych dzierżaw GPU, STT na CPU ≤ 5 s.

## Fake
`model-residency-fake`: budżety i tryby ze skryptu, deterministyczne eksmisje — do testów `voice-*` i `providers-local`.

## Otwarte pytania
- Pomiar realnego użycia VRAM na AMD (DXGI/ADLX) vs estymaty z manifestów modeli — do ustalenia w SPEC v1 po spike (h).
