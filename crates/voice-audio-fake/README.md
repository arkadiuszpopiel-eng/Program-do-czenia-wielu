# voice-audio-fake

Atrapa `voice-audio`: wirtualne urządzenia 48 kHz z **wirtualnym zegarem** (`FakeAudio::advance`, okresy 10 ms).

- Mikrofon odtwarza WAV (`load_wav`) albo wygenerowane próbki (`set_mic_signal`, opcjonalnie w pętli).
- Wyjście renderuje **ten sam mikser** co `-impl`; całość zapisywana (`recorded_output`).
- `set_echo(Some(EchoPath))` — pętla zwrotna głośnik → mikrofon (opóźnienie + FIR „pokoju”, deterministyczny)
  do testów AEC i barge-in; `open_loopback` — referencja awaryjna.
- Symulacje: opóźnienie wyjścia (`set_output_latency`), hot-plug (`plug`/`unplug`/`set_default` → `DeviceEvent`,
  zamknięcie strumieni odłączonego urządzenia), błędy otwarcia (`fail_next_open`: konflikt trybu wyłącznego,
  brak zgody na mikrofon).

Testy: `cargo test -p voice-audio-fake` (kontrakt współdzielony + echo + hot-plug + ducking na pętli).
Tylko `dev-dependencies` innych modułów.
