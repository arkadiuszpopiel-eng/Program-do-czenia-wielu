# tools-media — SPEC (v1: narzędzia zaimplementowane, F6)

## Cel
Multimedia dla agentek (PLAN §7.2): **informacje o pliku** audio/wideo/obrazu z nagłówków (czas, kodeki, wymiary — czysty Rust, bez ffmpeg), **konwersje** przez opcjonalny sidecar `ffmpeg` (zapis zawsze jako **nowy plik** przez `undo-journal`) i **odtwarzanie dźwięku** przez `voice-audio` w kolejce z mową agentek (ducking i zatrzymanie, gdy mówi użytkownik albo agentka).

## Fala i priorytet
F6, P1 (informacje, odtwarzanie WAV), P2 (konwersje — wymagają ręcznej instalacji ffmpeg).

## Kontrakt
```rust
media_info    { path }                                         → InfoOutput { path, info: MediaInfo { kind, format, mime, size_bytes, width?, height?, duration_ms?, sample_rate?, channels?, bits_per_sample?, bit_rate?, frames?, codecs, partial } }
media_convert { path, format: wav|mp3|flac|ogg|opus|m4a|mp4|webm|gif|png|jpg, output?, start_s?, duration_s?, max_side? (16–8192) }
                                                               → ConvertOutput { original, output, format, bytes, undo_step }
media_play    { path }                                         → PlayOutput { path, playback, duration_ms, queued, converted }
trait Transcoder  { available(); convert(&ConvertJob, cancel: Arc<AtomicBool>) -> Vec<u8> }   // aplikacja: FfmpegTranscoder (ExecPort)
trait AudioPlayer { async play(AudioClip, CancellationToken) -> PlayTicket; stop_all() }        // aplikacja: SpeakerPlayer (AudioIo + SchedulerLite)
pub struct MediaTools; MediaTools::new(MediaToolsDeps { fs, files: RangeRead, journal, transcoder, player, broker, env, deny, config, bus })
```
Zdarzenia: `tool.media.info` (rodzaj, format), `tool.media.convert` (format, krok cofania), `tool.media.play` (czas, kolejka) — bez ścieżek i treści.

## Zależności
`tools-common-contract`, `lib-media` (parsery: PNG, JPEG, GIF, BMP, WebP, WAV, MP3, AAC/ADTS, FLAC, Ogg, MP4/MOV/M4A/3GP/HEIF, Matroska/WebM, AVI), `undo-journal-contract`, `voice-audio-contract` (WAV, resampler, mikser), `scheduler-lite-contract` (zasób `speaker`), `platform-contract` (`FsPort`, `ExecPort`), `safety-broker-contract`, `compliance-contract`, `core-bus-contract`.

## Niezmienniki
- Ścieżki: postać i deny-lista (także po dowiązaniach) **przed** Brokerem; nagłówki czytane fragmentami z budżetem 8 MiB i limitem kroków/zagnieżdżeń — złośliwy plik nie wymusi pętli, przepełnienia ani odczytu gigabajtów (proptest + przypadki złośliwe).
- Konwersja: bez ffmpeg — błąd **bez** pytania właściciela o zgodę; formaty z listy zamkniętej (żadnych argumentów od modelu), wejście z rozpoznanym nagłówkiem i **wymuszonym demuxerem** (bez list odtwarzania HLS/concat i wzorców), `-protocol_whitelist file`, ścieżki jako `file:`, metadane usunięte, `-n`; proces w Job Object (pamięć 2 GiB, 10 min, środowisko bez dziedziczenia, kill-switch); wynik ≤ 512 MiB.
- Nowy plik nigdy nie nadpisuje: domyślnie `<nazwa> (Alfa).<ext>`, potem `(Alfa 2)`…; istniejący `output` albo równy źródłu → odmowa; zapis tylko przez dziennik cofania („Cofnij” usuwa plik).
- Odtwarzanie: klip mono (inne częstotliwości → 48 kHz), ≤ 10 min; czeka na dzierżawę `speaker` (priorytet narracji — po wypowiedzi agentki); wywłaszczenie → ducking −15 dB natychmiast, twardy stop ≤ 100 ms i zwolnienie głośnika; kill-switch schedulera, anulowanie przebiegu, `stop_all` → stop od razu. Tor głosu jako `Filler` (poza „usłyszanym prefiksem”).
- Wyniki to metadane z parsera (liczby i nazwy z listy) — bez taintu sesji; sekretów brak.

## Zdolności / uprawnienia
`media_info`, `media_play`: `fs.read(plik)`; `media_convert`: `fs.read(źródło)` + `fs.write(nowy plik)` — każdy token weryfikowany i zwalniany po akcji, fakt „dane prywatne”. Odwracalność `yes`; `media_convert` i `media_play` `mutating = true` (role tylko do odczytu ich nie dostają). Grupy ról: `media`, `media.read`, `media.write`, `media.play` (wbudowana: Wykonawczyni).

## Izolacja
`inproc`, `lazy`; ffmpeg jako proces potomny w Job Object (`ExecPort` Brokera); odczyty i konwersje na `spawn_blocking`; odtwarzanie w zadaniu tokio.

## Budżet zasobów
RAM ≤ 64 MB poza buforami pliku (WAV do odtworzenia ≤ 128 MiB, wynik konwersji ≤ 512 MiB — limity konfiguracji); `media_info` ≤ 50 ms dla nagłówka na dysku lokalnym.

## Konfiguracja (klucze TOML)
`[tools.media] max_play_ms = 600000`, `max_play_bytes = 134217728`, `max_output_bytes = 536870912` (dziś domyślne). Sidecar: `%LOCALAPPDATA%\Alfa\sidecars\ffmpeg\ffmpeg.exe` — pozycja `sidecar-ffmpeg` w katalogu „Modele i silniki” (instalacja ręczna, „do potwierdzenia”, bez pobierania automatycznego); katalog roboczy `media-work`.

## Wkład do UI
Pozycja ffmpeg na stronie „Modele i silniki” (stan, licencja, instrukcja instalacji ręcznej). Odtwarzanie słychać w głośniku Alfy; zatrzymuje je „Stop”/kill-switch.

## Testy akceptacyjne
- `ACC-F6-tools-media-01`: konwersja = nowy plik, źródło bajt w bajt bez zmian, „Cofnij” usuwa wynik; istniejący cel → odmowa (`tests/media.rs`).
- `ACC-F6-tools-media-02`: argumenty ffmpeg z listy zamkniętej, tylko `file:`, Job Object, środowisko bez sekretów, limit czasu, anulowanie, za duży wynik — bez plików tymczasowych (`tests/ffmpeg.rs`).
- `ACC-F6-tools-media-03`: odtwarzanie po mowie agentki, wywłaszczenie oddaje głośnik, kill-switch/anulowanie zatrzymują (`tests/player.rs`).
- `lib-media`: 400 przypadków losowych i mutacji × 3 testy właściwości, złośliwe rozmiary pudełek MP4, zagnieżdżenia, zera w mianownikach.

## Fake
`tools-media-fake`: `FakeTools`, `FakeTranscoder` (poprawne nagłówki formatu docelowego, „brak ffmpeg”), `FakePlayer` (zapis klipów, kolejka).

## Otwarte pytania
- Kamera i mikrofon jako narzędzia (PLAN §7.2 „Multimedia”) — po decyzji o prywatności nagrań.
- Pełny parser EBML (Matroska/WebM: ścieżki, czas) — dziś tylko rozpoznanie formatu.
- Wersja i źródło buildu ffmpeg (licencja LGPL/GPL) — do potwierdzenia przez człowieka przed wydaniem.
