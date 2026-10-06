# model-residency-contract

Kontrakt zarządcy rezydencji modeli w RAM/VRAM (docs/modules/model-residency/SPEC.md, PLAN §3.4–3.5).

- Typy: `Budget` (z `device-profile`: VRAM po rezerwie pulpitu, RAM, `stt_tts_exclusive`), `LeaseRequest`
  (`ModelRole` STT/TTS/LLM/embedder/VAD, `Priority` VoiceRt > Conversation > Background, `Placement`
  GpuOnly/GpuPreferred/GpuIfFree/CpuOnly, VRAM/RAM na GPU i RAM na CPU, `idle_unload_ms`), `Lease`, `Grant`,
  `Revocation` + `RevokeReason`, `Mode` (flagi gra/bateria + emulacja budżetu), `ResidencyState`.
- Trait `Residency` (synchroniczny, ≤ 1 ms): `acquire/release/touch/set_in_use/lease/snapshot/set_mode/
  set_budget/reap_idle/listen/refresh_mode(&dyn ModeSource)`; `LeaseListener` (odebranie, przeniesienie
  na CPU); `ModeSource` (pełny ekran/gra, bateria — sygnał z zewnątrz); `ResidencyClock` + `ManualClock`.
- Czysta maszyna stanów `LeaseTable` (wspólna dla `-impl`/`-fake`): wolne miejsce → eksmisja ustępujących
  (**LRU z priorytetami**: niższy priorytet albo równy i nieużywany), GPU przed CPU; `GpuIfFree` — GPU tylko
  bez wypierania, inaczej CPU (STT obok lokalnego LLM na laptopie 6 GB); wykluczenie STT/ciężki
  TTS na GPU przy ciasnym VRAM; gra → dzierżawy GPU na CPU albo eksmisja; bateria → bez tła; zmniejszenie
  budżetu → eksmisja od najniższego priorytetu.
- Zdarzenia `residency.granted/released/evicted/moved/mode_changed/oom_avoided/budget_exceeded` (`ResidencyEvent`).
- Testy: tabelaryczne (baseline 8 GB, laptop 6 GB, 8B obok STT, gra, bateria, emulacja), **property-based**
  (1000 losowych sekwencji: suma ≤ budżet po każdej operacji; `Wait` nigdy na niższy priorytet; wykluczenie
  STT/TTS; `GpuIfFree` wypiera z GPU tylko, gdy CPU się nie da), `contract-tests` uruchamiane na `-fake`
  i `-impl`.
