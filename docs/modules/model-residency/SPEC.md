# model-residency — SPEC (v1)

## Cel
Zarządca rezydencji modeli w RAM/VRAM: limity per maszyna, dzierżawy (lease) dla STT/TTS/LLM/embeddera/VAD, kolejność wymiany (STT+TTS+LLM nie zawsze naraz), zwalnianie po bezczynności, brak wyścigu o VRAM z grami (pełny ekran → STT/LLM na CPU lub chmurę), tryb baterii (PLAN §3.4, §3.5, §6.3).

## Fala i priorytet
F2. P0. (W F1 `providers-local` używa prostego limitu; migracja do lease w F2.)

## Kontrakt (v1 — `crates/model-residency-contract`)
```rust
pub struct Budget { pub vram_mb: u32 /* karta główna, po rezerwie pulpitu */, pub ram_mb: u32, pub desktop_reserve_mb: u32, pub stt_tts_exclusive: bool }
pub struct GpuId(pub u32);                 // = GpuInfo::id z device-profile (kolejność adapterów, stała w obrębie maszyny)
pub struct GpuPool { pub gpu: GpuId, pub vram_mb: u32, pub desktop_reserve_mb: u32, pub stt_tts_exclusive: bool, pub shared: bool /* pamięć z RAM */ }
pub struct GpuLayout { pub primary: GpuId, pub others: Vec<GpuPool> }   // GpuLayout::from_profile(&Profile)
pub struct LeaseRequest { pub owner: String, pub model: String, pub role: ModelRole /* Stt|Tts|Llm|Embedder|Vad */,
                          pub priority: Priority /* VoiceRt > Conversation > Background */, pub placement: Placement /* GpuOnly|GpuPreferred|GpuIfFree|CpuOnly */,
                          pub vram_mb: u32, pub ram_mb: u32 /* na GPU */, pub cpu_ram_mb: u32 /* na CPU */, pub idle_unload_ms: u64,
                          pub gpu: Option<GpuId> /* karta docelowa; None albo nieznana — główna */ }
pub struct Grant { pub lease: Lease /* id, device Gpu|Cpu, gpu: Option<GpuId>, in_use, last_used */, pub evicted: Vec<Revocation> }
pub trait Residency: Send + Sync {
    fn acquire(&self, r: LeaseRequest) -> Result<Grant, ResidencyError /* Wait{blockers} | TooLarge | Gaming | Battery | Invalid */>;
    fn release(&self, id: LeaseId) -> Result<(), ResidencyError>;
    fn touch(&self, id: LeaseId) -> Result<(), ResidencyError>;
    fn set_in_use(&self, id: LeaseId, in_use: bool) -> Result<(), ResidencyError>;
    fn lease(&self, id: LeaseId) -> Option<Lease>;
    fn snapshot(&self) -> ResidencyState;
    fn set_mode(&self, m: Mode /* { gaming, battery, emulated: Option<Budget> } */) -> Vec<Change>;
    fn set_budget(&self, b: Budget) -> Vec<Change>;   // karta główna + RAM
    fn set_gpus(&self, g: GpuLayout) -> Vec<Change>;  // karty maszyny; dzierżawy na zniknętych kartach odbierane
    fn set_hardware(&self, b: Budget, g: GpuLayout) -> Vec<Change>;  // zmiana sprzętu naraz (budżet egzekwowany raz)
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
Tryb (flagi łączą się): gra → nowe `GpuOnly` odrzucane, `GpuPreferred` na CPU, istniejące dzierżawy GPU (każda karta)
przenoszone na CPU (`moved`) albo eksmitowane; bateria → tło eksmitowane i odrzucane; emulacja → budżet = min(rzeczywisty,
emulowany), pozostałe karty też najwyżej tyle co emulowana (jak `Profile::emulate`).

**Pule kart (C6.6).** RAM jest jedną pulą maszyny, każda karta ma własną pulę pamięci: karta główna — `Budget` (rekomendacja
`device-profile` z nakładką `[machine.residency]`, bez zmian), pozostałe karty z obsługiwanym backendem —
`GpuPool::from_gpu`: karta z własną pamięcią (osobna albo APU z rezerwą z BIOS-u ≥ 5,5 GB) — pamięć minus 768 MB rezerwy
pulpitu, STT/ciężki TTS na zmianę poniżej 8 GB; zintegrowana z małą rezerwą — `GpuInfo::usable_memory_mb` (≤ ¼ RAM, ≤ 4 GB),
`shared` (dzierżawa na niej zajmuje też RAM), STT/TTS na zmianę. Dzierżawa na GPU zajmuje pulę karty z
`LeaseRequest::gpu` (brak albo karta bez puli — główna); bez przenoszenia na inną kartę (silnik jest przypięty do karty) —
gdy jej zabraknie miejsca, zostaje procesor albo `Wait`. Planowanie i eksmisje liczą zużycie „widziane z karty”: żądanie
na karcie B wypiera tylko dzierżawy z karty B albo — gdy brakuje RAM — zajmujące RAM; model rozmowy na karcie A nie
blokuje rozpoznawania mowy na karcie B. Wykluczenie STT/ciężki TTS dotyczy jednej karty. `ResidencyState.budget`/`used`
opisują nadal kartę główną i RAM (zgodność wstecz), `ResidencyState.gpus` — każdą kartę (`GpuState`: pula, zajęte, wolne).
Klucz karty to `GpuInfo::id`, nie LUID (LUID zmienia się po ponownym uruchomieniu systemu).

Zdarzenia: `residency.granted/released/evicted/moved`, `residency.mode_changed`, `residency.oom_avoided`, `residency.budget_exceeded`.

## Zależności
`core-bus/config/log-contract`, `device-profile-contract` (VRAM, bateria, pełny ekran), `platform-windows-contract` (pomiar użycia VRAM/RAM). Klienci: `voice-stt/tts/vad/turn`, `providers-local`, `search` (embedder).

## Niezmienniki
- Suma dzierżaw ≤ budżet maszyny minus rezerwa pulpitu — w **każdej** puli (RAM, każda karta); przekroczenie =
  `Wait`/`Evictable`, nigdy cichy OOM.
- Maszyna z jedną kartą (albo żądania bez wskazania karty) działa dokładnie jak przed podziałem na pule (test własności
  `requests_without_card_behave_like_single_card`).
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
- `ACC-F2-model-residency-01`: property-based — dowolna sekwencja acquire/release nie przekracza budżetu; brak zakleszczeń (1000 losowych scenariuszy);
  także na maszynie z trzema kartami (`tests/props_gpus.rs`, 500 scenariuszy: każda pula, brak wypierania z innej karty).
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

## Zmiany — osobny budżet pamięci na kartę (C6.6, 2026-10-18)
- Problem: jedna pula VRAM dla całej maszyny — model rozmowy na karcie A blokował (albo wypierał) rozpoznawanie mowy
  przypięte do karty B. Teraz każda karta ma własną pulę (`GpuLayout::from_profile`, `ResidencyManager::from_device`),
  RAM jest wspólny, a dzierżawa wskazuje kartę (`LeaseRequest::gpu`). Kontrakt zgodny wstecz dla klientów: brak karty =
  karta główna; nowe pole w literałach żądania (`gpu: None`), nowe metody traitu `set_gpus` i `set_hardware` z
  implementacją domyślną (dodatek bez łamania implementacji; `-impl` i `-fake` robią to atomowo na `LeaseTable`).
  Dzierżawy bez wskazanej karty idą za kartą główną, gdy `set_gpus` zmienia kartę główną (np. umowna `GpuId(0)` z
  `LeaseTable::new` → RTX `GpuId(1)` na laptopie), zamiast zostać na karcie o starym numerze. Zmienione sygnatury pomocników `plan`/`shrink_victims`/`exclusive_conflicts`
  (miejsce `Option<GpuId>` i `&Pools` zamiast `Device` i `Budget`) — poza kontraktem nikt ich nie używał.
- (Do 0.0.64 klienci nie wskazywali kart — `gpu: None` wszędzie; od 0.0.65 — niżej.) Następne kroki (po próbie na
  sprzęcie): Qwen3-ASR na karcie AMD/Intel poniżej 12 GB z dzierżawą na karcie, gdy model rozmowy z niej nie korzysta
  (dziś reguła `VULKAN_QWEN_CARD_MIN_MB` w `app-modules::stt` — procesor); model pomocniczy na karcie zintegrowanej
  (`providers-local`, `shared_memory`) z dzierżawą w puli tej karty zamiast „na procesorze”; rezerwa 1,5 GB na mowę obok
  modelu rozmowy niepotrzebna, gdy mowa liczy na innej karcie.
- Testy: `gpus_tests.rs` (pule z profili: dwie osobne karty, laptop Intel UHD + RTX 4050, APU z małą i dużą rezerwą,
  karta bez backendu), `tests_gpus.rs` (scenariusze), kontraktowy `gpus_have_separate_budgets` (na `-impl` i `-fake`),
  `props_gpus.rs` (własności, w tym równoważność z jedną kartą).

## Zmiany — klienci wskazują kartę, zmiana sprzętu przelicza pule (C6.6, 0.0.65)
- Karta silnika: `card_for(&Profile, Backend, Option<&str>) -> Option<GpuId>` (kontrakt, czysta funkcja) — ta sama
  decyzja co wybór karty silnika: karta przypięta po nazwie (`--device` `llama-server`, `LocalConfig::gpu_name`,
  tylko przy `pin_device` i karcie obsługującej backend — inaczej serwer startuje bez przypięcia; nazwy porównuje
  wspólne `same_card_name`, którego używa też `providers-local::devices::device_for`), inaczej karta główna, gdy
  obsługuje backend, inaczej największa karta z tym backendem (CUDA przy głównej AMD); `Backend::Cpu` i maszyna bez
  kart — `None`.
- Klienci: `providers-local` — dzierżawa LLM na karcie serwera (`target_gpu`: CUDA/Vulkan → `card_for` z `gpu_name`),
  budżet warstw z puli tej karty (`ResidencyState::gpu`), model pomocniczy na karcie zintegrowanej (`shared_memory`)
  nadal w RAM bez karty; `voice-stt` — `WhisperStt::with_cards(&Profile)`: CUDA i Vulkan → `card_for`, procesor —
  bez karty (`app-voice` `SystemVoice::with_device`); `voice-tts` — `tts_lease_request(…, dml: Option<u32>, vram_mb)`:
  numer adaptera DXGI Supertonic (`--dml n`) = `GpuInfo::id` → `GpuId(n)`, `GpuIfFree`, bez karty — CPU;
  `voice-vad` i embedder (`lib-embed`) liczą na procesorze — `gpu: None` bez zmian. Maszyna z jedną kartą: karta
  wskazana = główna, zachowanie identyczne.
- Zmiana sprzętu: `ResidencyConfig::hardware(&dyn DeviceProfile)` (budżet z nakładką + `GpuLayout::from_profile`),
  `ResidencyManager::sync_hardware` (porównuje z `LeaseTable::base_budget`/`gpus`; różnica → `set_hardware`,
  inaczej `None`), `ResidencyModule::with_hardware(device, config)`: po każdym zdarzeniu `device.*` z magistrali
  (`device.changed`, `device.override`, …) i w każdym kroku zadania tła (zapas dla zmian bez zdarzenia, np. emulacji).
  Kompozycja: `app_modules::route::local::residency`.
- Numer karty (`GpuId` = `GpuInfo::id`) to pozycja adaptera DXGI, nie tożsamość. `sync_hardware` pamięta nazwę i LUID
  karty pod każdym numerem; gdy pod tym samym numerem jest inna karta (zmiana kolejności adapterów, np. po zmianie
  wyświetlacza w laptopie hybrydowym albo resecie sterownika — LUID się zmienia), najpierw przelicza pule bez
  podmienionych numerów (dzierżawy wskazujące je są odbierane jako „na karcie, której nie ma”; podmieniona karta
  główna — numer zastępczy), potem z nowym układem (dzierżawy idące za kartą główną przechodzą na nią). Bez tego
  dzierżawa z jawną kartą zostałaby pod starym numerem, czyli na obcej karcie. Test:
  `hardware.rs::reordered_adapters_revoke_leases_pinned_by_number`. Ograniczenia: zarządca bez profilu
  (`with_gpus`) nie zna tożsamości kart; dwie karty o tej samej nazwie bez znanego LUID zamienione miejscami nie są
  rozpoznawane; reset sterownika ze zmianą LUID odbiera dzierżawy także wtedy, gdy kolejność się nie zmieniła
  (ostrożnie — klienci zakładają je ponownie).
- Testy: `gpus_tests.rs::engine_card_follows_the_engine_choice`, `model-residency-impl/tests/hardware.rs` (laptop
  Intel UHD 0 + RTX 4050 1: STT na CUDA i model rozmowy na karcie 1, VAD na CPU; dwie karty AMD: mowa na karcie B nie
  blokuje rozmowy na karcie A; zmiana sprzętu odbiera dzierżawę z karty, której nie ma; moduł po `device.changed`),
  `voice-stt-impl/tests/stt_cards.rs`, `voice-tts-impl/tests/tts_cards.rs`, `voice-vad-impl/tests/vad.rs`,
  `providers-local-impl/tests/laptop.rs` (dzierżawa LLM i STT na RTX).
- Poza zakresem: Qwen3-ASR (`QwenStt`) i Supertonic w aplikacji nie zakładają dzierżaw (zajmują kartę poza zarządcą) —
  dzierżawa z kartą `main_card`/`tts_dml_device` po próbie na sprzęcie; `device-profile` kolejkuje `device.changed`
  w `refresh()`, ale publikuje je dopiero `poll_changes()`, którego aplikacja nie wywołuje — do tego czasu zmianę
  łapie krok zadania tła (co `tick`, domyślnie 30 s).
