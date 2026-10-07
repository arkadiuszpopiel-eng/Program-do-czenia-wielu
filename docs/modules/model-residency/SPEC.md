# model-residency — SPEC (v1)

## Cel
Zarządca rezydencji modeli w RAM/VRAM: limity per maszyna, dzierżawy (lease) dla STT/TTS/LLM/embeddera/VAD, kolejność wymiany (STT+TTS+LLM nie zawsze naraz), zwalnianie po bezczynności, brak wyścigu o VRAM z grami (pełny ekran → STT/LLM na CPU lub chmurę), tryb baterii (PLAN §3.4, §3.5, §6.3).

## Fala i priorytet
F2. P0. (W F1 `providers-local` używa prostego limitu; migracja do lease w F2.)

## Kontrakt (v1 — `crates/model-residency-contract`)
```rust
pub struct Budget { pub vram_mb: u32 /* po rezerwie pulpitu */, pub ram_mb: u32, pub desktop_reserve_mb: u32, pub stt_tts_exclusive: bool }
pub struct LeaseRequest { pub owner: String, pub model: String, pub role: ModelRole /* Stt|Tts|Llm|Embedder|Vad */,
                          pub priority: Priority /* VoiceRt > Conversation > Background */, pub placement: Placement /* GpuOnly|GpuPreferred|GpuIfFree|CpuOnly */,
                          pub vram_mb: u32, pub ram_mb: u32 /* na GPU */, pub cpu_ram_mb: u32 /* na CPU */, pub idle_unload_ms: u64 }
pub struct Grant { pub lease: Lease /* id, device Gpu|Cpu, in_use, last_used */, pub evicted: Vec<Revocation> }
pub trait Residency: Send + Sync {
    fn acquire(&self, r: LeaseRequest) -> Result<Grant, ResidencyError /* Wait{blockers} | TooLarge | Gaming | Battery | Invalid */>;
    fn release(&self, id: LeaseId) -> Result<(), ResidencyError>;
    fn touch(&self, id: LeaseId) -> Result<(), ResidencyError>;
    fn set_in_use(&self, id: LeaseId, in_use: bool) -> Result<(), ResidencyError>;
    fn lease(&self, id: LeaseId) -> Option<Lease>;
    fn snapshot(&self) -> ResidencyState;
    fn set_mode(&self, m: Mode /* { gaming, battery, emulated: Option<Budget> } */) -> Vec<Change>;
    fn set_budget(&self, b: Budget) -> Vec<Change>;
    fn reap_idle(&self) -> Vec<Revocation>;
    fn listen(&self, owner: &str, l: Arc<dyn LeaseListener /* revoked, moved */>);
    fn refresh_mode(&self, source: &dyn ModeSource /* fullscreen_active, on_battery */) -> Vec<Change>;
}
```
Reguły (czysta maszyna stanów `LeaseTable`, wspólna dla `-impl`/`-fake`): najpierw wolne miejsce, potem eksmisja
ustępujących — **LRU z priorytetami**: dzierżawa ustępuje, gdy ma niższy priorytet albo równy i nie jest w użyciu
(`in_use`); `GpuPreferred`: GPU bez eksmisji → GPU z eksmisją → CPU; `GpuIfFree`: GPU bez wypierania (dozwolona
tylko wymiana STT ↔ ciężki TTS) → CPU (bez eksmisji, potem z eksmisją) → GPU z wypieraniem (ostatnia możliwość — głos
nie czeka na niższy priorytet); `Wait` tylko na blokery o priorytecie ≥ żądania.
Tryb (flagi łączą się): gra → nowe `GpuOnly` odrzucane, `GpuPreferred` na CPU, istniejące dzierżawy GPU przenoszone na CPU
(`moved`) albo eksmitowane; bateria → tło eksmitowane i odrzucane; emulacja → budżet = min(rzeczywisty, emulowany).

Zdarzenia: `residency.granted/released/evicted/moved`, `residency.mode_changed`, `residency.oom_avoided`, `residency.budget_exceeded`.

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
Per maszyna: `[machine.residency] vram_mb = "auto"`, `ram_mb = "auto"`, `desktop_reserve_mb = 768`, `gaming_mode = "auto" | "off"`, `battery_mode = "auto" | "off"`, `tick = "30s"` (bezczynność i odświeżenie trybu; limit bezczynności podaje klient w `idle_unload_ms`); wspólne: `[residency] priority_order = ["voice_rt", "conversation", "background"]`.

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

## Zmiany — fala 6, laptop 6 GB (2026-10-06)
- `Placement::GpuIfFree` — dla modeli z użyteczną wersją CPU, których załadowanie nie powinno wyrzucać z karty modelu
  potrzebnego w tej samej rozmowie. Używa jej STT (`voice-stt-impl`): na laptopie RTX 4050, gdy whisper CUDA nie mieści
  się obok lokalnego LLM, STT idzie na CPU zamiast wypierać LLM (wcześniej `GpuPreferred` + `VoiceRt` wypierało LLM,
  który przeładowywał się na CPU — najgorszy wariant: wolny LLM i przeładowanie w trakcie rozmowy). Właściwości
  (`tests/props.rs`): wypieranie z GPU przy `GpuIfFree` tylko, gdy żądanie nie mieści się na CPU.
- Dzierżawa LLM liczy KV cache dla kontekstu uruchomienia i tylko warstwy na karcie (częściowe odciążenie — SPEC
  `providers-local`), więc suma dzierżaw odpowiada realnej pamięci karty; `providers-local` oznacza dzierżawę
  `in_use` w trakcie żądania i odświeża ją po nim. Laptop 6 GB (5153 MB): Bielik 4.5B Q8_0 3570 MB + STT 1500 MB.
- Testy: kontraktowy `gpu_if_free_does_not_preempt` (na `-impl` i `-fake`), tabelaryczny
  `gpu_if_free_swaps_tts_and_preempts_only_when_cpu_is_impossible`, scenariusz laptopa w
  `providers-local-impl/tests/laptop_voice.rs` i `voice-stt-impl/tests/stt.rs`.

