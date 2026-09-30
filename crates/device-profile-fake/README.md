# device-profile-fake

Deterministyczna atrapa `DeviceProfile` (docs/modules/device-profile/SPEC.md, sekcja „Fake”)
do testów `model-residency`, `voice-*`, `providers-local`, `notify` bez sprzętu. Startuje z profilu
(`FakeDeviceProfile::baseline()/desktop()/laptop()` z `device_profile_contract::fixtures`),
a test steruje zdarzeniami: `set_power` (sieć ↔ bateria), `set_fullscreen`, `hot_plug` (nowy
sprzęt, ten sam `MachineId`), `set_overlay` (nadpisanie użytkownika). Każda zmiana zapisuje
`DeviceEvent` do odczytu przez `drain_events()`. Rekomendacje liczą te same reguły kontraktu co `-impl`.
