# device-profile-impl

Implementacja modułu `device-profile` (docs/modules/device-profile/SPEC.md, PLAN §3.5, §6.3).
`DeviceProfileService::detect(hw, windows, config)` czyta sprzęt przez `HardwarePort`
(`platform-contract`): na Windows podaje go kompozycja (`platform-windows-impl`: DXGI, DXCore,
`GetSystemPowerStatus`, MMDevice, rejestr `MachineGuid`), poza Windows `SysinfoProbe`
(`sysinfo`: CPU/RAM; L3 i bateria z `/sys`; GPU brak). Moduł nie ma własnego kodu OS.
`MachineId` = SHA-256 z separacją domeny nad `MachineGuid`/`/etc/machine-id`, a bez nich nad
losowym UUID zapisanym raz w `state_dir/machine-id` — stabilny, bez danych osobowych.
Rekomendacje (`recommend`) i emulacja baseline (`emulate`) używają reguł kontraktu.
Zdarzenia `device.*` publikuje `start` (`device.detected`) i `poll_changes` (zasilanie, pełny
ekran, hot-plug po `refresh`, nadpisanie przez `set_overlay`). Nasłuch `WM_POWERBROADCAST`
bez pollingu — F2.
