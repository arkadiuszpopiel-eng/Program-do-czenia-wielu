# model-residency-impl

Zarządca rezydencji modeli (docs/modules/model-residency/SPEC.md), manifest `module.toml`
(`model-residency`, `inproc`, `always`).

- `ResidencyManager` — `Residency` na `LeaseTable` kontraktu pod muteksem, zegar monotoniczny, słuchacze
  właścicieli wywoływani poza blokadą, kolejka zdarzeń; `from_device(DeviceProfile, &ResidencyConfig)` —
  budżet karty głównej i RAM z rekomendacji `device-profile`, osobne pule pozostałych kart
  (`GpuLayout::from_profile`) + tryb z bieżących sygnałów; `with_gpus(Budget, GpuLayout, clock)`.
- `DeviceSignals` — `ModeSource` nad `device-profile` (pełny ekran → tryb gry, bateria).
- `ResidencyConfig` — `[machine.residency]`: `vram_mb`/`ram_mb` = liczba albo `"auto"`,
  `desktop_reserve_mb`, `gaming_mode`/`battery_mode` = `"auto"|"off"`, `tick = "30s"`.
- `ResidencyModule` — zdarzenia `residency.*` na magistralę w kolejności; zadanie tła co `tick`:
  zwalnianie bezczynnych dzierżaw i odświeżenie trybu.

Testy: zestaw kontraktowy, budżety z profili baseline/laptop (laptop: osobna pula karty zintegrowanej),
sygnały pełnego ekranu i baterii,
zdarzenia na `FakeBus`, konfiguracja, zadanie tła w czasie wirtualnym tokio.
