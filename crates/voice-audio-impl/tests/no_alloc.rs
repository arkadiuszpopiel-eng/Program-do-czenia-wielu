//! ACC-F2-voice-audio-04: ścieżka wątku RT (render miksera, konwersja do bajtów urządzenia,
//! zapis przechwytywania) nie alokuje. Osobny plik testu = osobny proces z licznikiem alokacji
//! (jeden test, bez równoległych wątków testowych).

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::alloc::System;
use std::time::Duration;

use stats_alloc::{INSTRUMENTED_SYSTEM, Region, StatsAlloc};
use voice_audio_contract::mixer::mixer;
use voice_audio_contract::{AudioFormat, Ducking, Lane, MediaTime, MixerConfig, capture_ring};
use voice_audio_impl::convert::{f32_to_le_bytes, le_bytes_to_f32};

#[global_allocator]
static GLOBAL: &StatsAlloc<System> = &INSTRUMENTED_SYSTEM;

#[test]
fn rt_path_does_not_allocate() {
    let (mut control, mut render) = mixer(MixerConfig::new(48_000));
    let (mut writer, _reader) = capture_ring(AudioFormat::mono(48_000), 10, 1_000);
    let mut out = vec![0.0f32; 960];
    let mut bytes = vec![0u8; 960 * 4];
    let mut captured = vec![0.0f32; 480];
    control.enqueue(Lane::Voice, 1, &vec![0.3; 48_000]).unwrap();
    control
        .enqueue(Lane::Effects, 2, &vec![0.1; 4_800])
        .unwrap();
    control.duck(Ducking::default()).unwrap();
    // Rozgrzewka (pierwsze wywołanie może zainicjować leniwe struktury bibliotek).
    render.render(&mut out, 2, MediaTime::ZERO);
    control
        .stop(Lane::Effects, Duration::from_millis(5))
        .unwrap();
    control.end(Lane::Voice, 1).unwrap();

    let region = Region::new(GLOBAL);
    for i in 0..200u64 {
        render.render(&mut out, 2, MediaTime::from_ms(10 * i));
        let n = f32_to_le_bytes(&out, &mut bytes);
        le_bytes_to_f32(&bytes[..n / 2], &mut captured);
        writer.write(&captured, MediaTime::from_ms(10 * i));
    }
    let stats = region.change();
    assert_eq!(stats.allocations, 0, "{stats:?}");
    assert_eq!(stats.reallocations, 0, "{stats:?}");
    assert_eq!(stats.deallocations, 0, "{stats:?}");
}
