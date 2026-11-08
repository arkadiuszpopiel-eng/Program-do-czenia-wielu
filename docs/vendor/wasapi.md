# wasapi — audio Windows (`voice-audio-impl`)

- Wersja: **`wasapi` 0.24.0** (MIT), zależy od `windows`/`windows-core` **0.62** — ta sama wersja co
  `platform-windows-impl` (reguła „jeden windows-rs”; w `deny.toml` `wasapi` jest wrapperem `windows`).
  Tylko `[target.'cfg(windows)'.dependencies]`. Docs: https://docs.rs/wasapi/0.24.0 · zweryfikowano
  `cargo clippy --target x86_64-pc-windows-msvc` 2026-10-01 (uruchomienie — self-hosted).

## Używane API
```rust
wasapi::initialize_mta();                                   // COM w KAŻDYM wątku, który dotyka obiektów
let e = DeviceEnumerator::new()?;
e.get_default_device(&Direction::Render)? / e.get_device(id)? / e.get_device_collection(&Direction::Capture)?;
dev.get_id()?; dev.get_friendlyname()?; dev.get_device_format()?;   // WaveFormat: get_samplespersec, get_nchannels
let mut c = dev.get_iaudioclient()?;
let (def_hns, _min) = c.get_device_period()?;
c.initialize_client(&WaveFormat::new(32, 32, &SampleType::Float, rate, ch, None), &Direction::Render,
    &StreamMode::EventsShared { autoconvert: true, buffer_duration_hns: def_hns * 2 })?;
// pętla zwrotna: klient z urządzenia Render + initialize_client(.., &Direction::Capture, ..)
let h = c.set_get_eventhandle()?; h.wait_for_event(ms)?;      // Err(EventTimeout) przy przekroczeniu
c.get_available_space_in_frames()?; c.get_buffer_size()?; c.start_stream()?; c.stop_stream()?;
c.get_audiorenderclient()?.write_to_device(frames, &bytes, None)?;     // bytes.len() == frames * blockalign
let cc = c.get_audiocaptureclient()?; cc.get_next_packet_size()?;     // Some(n) w trybie współdzielonym
let (frames, info) = cc.read_from_device(&mut bytes)?;                 // info.timestamp: QPC w 100 ns, info.flags.silent
let clock = c.get_audioclock()?; clock.get_frequency()?; clock.get_position()?;  // (pozycja, QPC w 100 ns)
let mut cb = DeviceEventCallbacks::new(); cb.set_device_added_callback(|id| ..); cb.set_default_device_callback(|dir, role, id| ..);
let _reg = e.register_notification_callback(cb)?;                      // rejestracja żyje, dopóki `_reg` istnieje
```

## Pułapki
- Obiekty `wasapi` (COM) nie są `Send` — wątek, który je tworzy, jest ich właścicielem (render/capture/powiadomienia).
- `GetStreamLatency` nie jest wystawione — opóźnienie wyjścia liczymy z `IAudioClock` (zapisane − odtworzone).
- Błędy: `WasapiError::Windows(e)` → `e.code().0 as u32`: `0x8889000A` (urządzenie zajęte, tryb wyłączny),
  `0x80070005` (brak zgody na mikrofon), `0x88890004` (urządzenie unieważnione/odłączone).
- `write_to_device` loguje `trace!` (crate `log`) — bez zarejestrowanego loggera koszt pomijalny.
- MMCSS (`AvSetMmThreadCharacteristicsW`) nie jest w `wasapi` — do dodania w `platform-windows-impl`.
