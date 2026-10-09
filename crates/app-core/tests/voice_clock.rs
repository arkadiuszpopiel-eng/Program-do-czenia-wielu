//! Zegar potoku głosowego a znaczniki czasu prawdziwego mikrofonu (błąd z laptopa, 2026-10-09:
//! mikrofon słuchał, a odpowiedź nie przychodziła). Koniec wypowiedzi to „teraz − koniec mowy”,
//! więc „teraz” musi być liczone w tej samej podstawie czasu co ramki z WASAPI. Test otwiera
//! prawdziwe urządzenie; bez mikrofonu (CI) kończy się bez sprawdzenia.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::time::{Duration, Instant};

use voice_audio_contract::StreamConfig;

#[test]
fn device_clock_shares_the_time_base_of_microphone_frames() {
    let audio = voice_audio_impl::system_audio();
    let Ok(mut input) = audio.open_input(None, &StreamConfig::input_default()) else {
        eprintln!("brak mikrofonu — test pominięty");
        return;
    };
    let started = Instant::now();
    let frame = loop {
        if let Some(frame) = input.read() {
            break frame;
        }
        if started.elapsed() > Duration::from_secs(5) {
            eprintln!("mikrofon nie dał ramek — test pominięty");
            return;
        }
        std::thread::sleep(Duration::from_millis(10));
    };
    let now_ms = platform_windows_impl::qpc_nanos() / 1_000_000;
    let frame_ms = frame.ts.as_ms();
    let skew = now_ms.abs_diff(frame_ms);
    eprintln!("teraz {now_ms} ms, ramka {frame_ms} ms, różnica {skew} ms");
    assert!(
        skew < 2_000,
        "zegar potoku ({now_ms} ms) i znacznik ramki ({frame_ms} ms) mają różne podstawy czasu"
    );
    // Zegar liczony od startu procesu (poprzednie zachowanie) byłby odległy o czas pracy systemu.
    assert!(
        frame_ms > 60_000,
        "znacznik ramki nie jest czasem od startu systemu: {frame_ms}"
    );
}
