# device-profile-contract

Kontrakt modułu `device-profile` (docs/modules/device-profile/SPEC.md, PLAN §3.5, §6.3).
`Profile` opisuje maszynę (CPU, RAM, GPU z backendami, NPU, bateria, zasilanie, audio, emulacja),
`MachineId` to stabilny skrót (32 hex) bez danych osobowych. Czyste reguły wspólne dla `-impl`
i `-fake`: `classify` (Baseline / Standard-AMD / Laptop-CUDA / Mocny / Unknown), `recommend`
(profil głosu A–D z wariantem, backend i model STT, lokalny LLM, budżet `model-residency`,
tryb baterii, kompromisy po polsku), `apply_overlay` (nakładka `config/machine/<id>.toml`)
i `Profile::emulate_baseline()` (6c/12t, 16 GB, VRAM 8 GB + korekta GPU ×2,2 / CPU +25% dla
szybszego gospodarza). Zdarzenia `device.*` (`DeviceEvent`), JSON Schema (`profile_schema` …),
fixture'y maszyn z planu (`fixtures::{baseline, desktop, laptop, laptop_on_battery}`)
i testy kontraktowe pod feature `contract-tests`.
