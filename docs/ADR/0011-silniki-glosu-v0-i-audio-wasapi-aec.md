# ADR 0011 — Silniki głosu v0 i audio: crate `wasapi`, AEC z własną referencją TTS

| Pole | Wartość |
|---|---|
| Status | Zaakceptowany (kandydaci TTS/STT do potwierdzenia w Voice Lab — spike (e), (a), (h) w F0) |
| Data | 2026-09-30 |
| Powiązane sekcje planu | §1.2 (Audio, ML), §6.2–6.8, §6.10, §16.2 (F0, F2), §17 |

## Kontekst

Rozmowa pełnodupleksowa po polsku z barge-in wymaga: niskiego opóźnienia audio, AEC (agentka mówi przez głośniki, mikrofon ją słyszy), czterech odrębnych głosów bez kluczy (profil A na baseline AMD 8 GB), wymienialności każdego elementu. Jakość polskiego TTS/STT konkretnych modeli jest hipotezą z źródeł wtórnych, niezweryfikowaną odsłuchem. AI nie słyszy jakości głosu — potrzebna bramka ludzka.

## Decyzja

| Element | Wybór v0 | Kandydaci / uwagi |
|---|---|---|
| Audio I/O | **crate `wasapi`** (loopback, tryb zdarzeniowy, per-proces), wątek RT, in-proc, **bez Wasm/IPC w callbacku audio** | `cpal` odrzucony (ograniczona kontrola bufora, problemy z loopbackiem) |
| AEC | **własny strumień TTS jako referencja** (crate `aec3`/`sonora`); zapas: loopback procesowy (Win10 20348+) lub tryb Communications | spike (a) porównuje trzy warianty; tryb Communications domyślnie tłumi inne strumienie o 80% |
| NS / AGC | RNNoise (DeepFilterNet3 jako opcja) | `voice-dsp`, in-proc |
| VAD | Silero VAD (MIT) / TEN VAD, CPU | `voice-vad` |
| Koniec tury | Smart Turn v3.2 (PL wśród 23 języków, ~8 MB, ~10 ms CPU) + regulowana cierpliwość | `voice-turn` |
| STT | whisper.cpp large-v3-turbo Q5_0 z bramką VAD (Vulkan/CUDA, fallback CPU); tryb dwuprzebiegowy | ADR 4; chmura: Scribe v2 RT, Soniox, gpt-4o-transcribe (po kluczach) |
| TTS v0 (bez kluczy) | **Pocket TTS + model PL społeczności** (CPU, RTF ≈ 0,21, ~200 ms do 1. fragmentu), Piper pl_PL jako zapas; **≥ 2 różne bazowe mówczynie + modyfikacja wysokości i tempa → 4 głosy v0** o sprawdzonej licencji | docelowo klon z 15–30 s referencji po castingu (Voice Design w chmurze → odsłuch ślepy A/B); Chatterbox/XTTS/F5 tylko profil D (NVIDIA) |
| Wake / adresowanie v0 | PTT/przełącznik (`WH_KEYBOARD_LL`) + adresowanie po imieniu z transkryptu | słowa wywoławcze „Hej Alfa/…" dopiero w F5 po FAR/FRR |
| Komendy szybkie | `voice-cmd` bez LLM: „stop", „pauza", „głośniej", „przełącz na Deltę"…; KWS „stop/czekaj" < 300 ms | „nie" przerywa tylko jako samodzielne słowo z pauzami, tylko w `Speaking` |

Zatrzymanie dwustopniowe: ducking −15 dB < 50 ms → twardy stop po ≥ 150–250 ms potwierdzonej mowy (stop TTS, anulowanie LLM, czyszczenie kolejki), realnie ≤ ~400 ms. Głośnik = zasób wyłączny w Schedulerze (jedna agentka naraz).

## Alternatywy (odrzucone)

| Alternatywa | Dlaczego nie |
|---|---|
| `cpal` | brak niezawodnego loopbacku WASAPI, słaba kontrola bufora |
| AEC wyłącznie z loopbacku systemowego | loopback zawiera inne aplikacje i opóźnienie urządzenia; własna referencja TTS jest najdokładniejsza |
| Chatterbox / XTTS-v2 / F5 jako TTS domyślny | PyTorch-ROCm niestabilny na baseline AMD; RTF ≥ 1; zgłoszony akcent PL (Chatterbox) |
| Kokoro, Orpheus, Kyutai, Dia, Sesame, Moonshine, Voxtral RT, Aura-2 | brak polskiego (wg badania) |
| openWakeWord jako słowa wywoławcze od startu | tylko angielski; własny trening dopiero w F5 |
| Chmurowy TTS jako jedyny | profil A musi działać bez kluczy |

## Konsekwencje

- Cztery moduły in-proc na wątku RT (`voice-audio/dsp/vad/turn`) i dwa sidecary (`voice-stt`, `voice-tts`); `model-residency` pilnuje, by STT+TTS+LLM nie zajmowały VRAM naraz.
- Kryteria F2 (zestaw zamrożony z korpusu własnego): WER PL ≤ 12%; recall „stop/anuluj" ≥ 99% przy reakcji < 300 ms; fałszywe przerwania ≤ 1/godz. (głośniki laptopa + tło TV); prefiks ±1 słowo ≥ 90%; odrębność głosów cos-sim ECAPA ≤ 0,6 + ABX ≥ 90%.
- Cele opóźnień (wstępne): A p50 ≤ 2000 / p95 ≤ 3000 ms; B p50 ≤ 1300 / p95 ≤ 2000 ms; C p50 ≤ 900 / p95 ≤ 1500 ms — pomiar loopbackiem od ostatniej ramki mowy do pierwszej próbki na wyjściu.
- Bramki ludzkie: korpus własny (~30–45 min), odsłuchy castingu, testy akustyczne na obu maszynach (najtrudniej: wbudowany mikrofon i głośniki laptopa).
- Wszystkie oceny jakości PL są hipotezami do zmierzenia w Voice Lab; „nowszy = lepszy" nie jest zakładane.

## Jak cofnąć

- Każdy silnik jest `-impl` za kontraktem (`voice-tts-contract`, `voice-stt-contract`); wymiana kandydata po Voice Lab nie dotyka `voice-dialog`.
- Gdyby `wasapi` okazał się niewystarczający, alternatywą jest własna warstwa na windows-rs (IAudioClient3) w `platform-windows` — ten sam kontrakt `voice-audio`.
- No-go castingu (średnia ocen TTS < 4,0) nie blokuje F1 — zostają głosy v0.

## Aktualizacja 2026-10-07 — `wasapi` 0.24.0 → 0.25.0 (RUSTSEC-2026-0332)

- **Powód:** ostrzeżenie [RUSTSEC-2026-0332](https://rustsec.org/advisories/RUSTSEC-2026-0332) (`unsound`):
  w 0.24.0 bezpieczna funkcja `WaveFormat::parse(&WAVEFORMATEX)` czyta poza nagłówkiem, gdy
  `wFormatTag = WAVE_FORMAT_EXTENSIBLE`. `cargo deny` (CI) odrzuca 0.24.0.
- **Zmiana:** 0.25.0 (poprawka `2562db7`): `parse` jest teraz `unsafe fn` na wskaźniku; bezpieczna
  alternatywa to `WaveFormat::parse_from_blob_bytes`. `voice-audio-impl` nie woła `parse` — reszta
  używanego API (`DeviceEnumerator`, `AudioClient`, `WaveFormat::new`, `get_device_format`, strumień
  zdarzeniowy) bez zmian; 0.25.0 nadal zależy od `windows` 0.62 (jedna wersja windows-rs w workspace).
- **Sprawdzenie:** `cargo clippy -p voice-audio-impl --target x86_64-pc-windows-msvc -D warnings`,
  `cargo deny check`, testy `voice-audio-impl` (Linux) i job „Rust (windows-latest)” w CI.
