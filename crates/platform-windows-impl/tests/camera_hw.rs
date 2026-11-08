//! Test sprzętowy kamer (Media Foundation) — wymaga prawdziwej kamery, dlatego `#[ignore]`:
//! `cargo test -p platform-windows-impl --test camera_hw -- --ignored --nocapture`.
//! Klatki nie są zapisywane na dysk ani wypisywane; test drukuje tylko nazwy urządzeń, rozmiar
//! klatki, czasy i liczbę różnych wartości pikseli.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::print_stdout)]

use std::time::{Duration, Instant};

use platform_contract::{CameraPort, PlatformError};
use platform_windows_impl::WinCameras;

#[test]
fn listing_never_fails_on_windows() {
    let devices = WinCameras.devices();
    if cfg!(windows) {
        assert!(devices.is_ok(), "lista kamer: {devices:?}");
    } else {
        assert!(matches!(devices, Err(PlatformError::Unsupported(_))));
    }
}

#[test]
#[ignore = "wymaga kamery; uruchamiany ręcznie na sprzęcie"]
fn first_camera_delivers_changing_frames() {
    let cameras = WinCameras;
    let started = Instant::now();
    let devices = cameras.devices().expect("lista kamer");
    println!("kamery ({} w {:?}):", devices.len(), started.elapsed());
    for device in &devices {
        println!(
            "  „{}” — id o długości {} znaków",
            device.name,
            device.id.len()
        );
    }
    let first = devices.first().expect("brak kamery w systemie");
    assert!(!first.id.is_empty() && !first.name.is_empty());

    let started = Instant::now();
    let mut stream = match cameras.open(&first.id, 640) {
        Ok(stream) => stream,
        Err(e) => panic!("nie można otworzyć kamery „{}”: {e}", first.name),
    };
    println!("otwarcie + pierwsza klatka: {:?}", started.elapsed());
    let mut sizes = Vec::new();
    let mut previous: Option<Vec<u8>> = None;
    let mut changed = 0;
    for index in 0..5 {
        let at = Instant::now();
        let frame = stream.read(Duration::from_secs(5)).expect("klatka");
        assert_eq!(
            frame.pixels.len(),
            frame.width as usize * frame.height as usize * 4
        );
        assert!(frame.width >= 64 && frame.height >= 48, "rozmiar klatki");
        assert!(frame.width.max(frame.height) <= 640, "limit dłuższego boku");
        assert_eq!((frame.width, frame.height), stream.size());
        let mut values = [false; 256];
        for pixel in frame.pixels.chunks_exact(4) {
            values[usize::from(pixel[0])] = true;
            values[usize::from(pixel[1])] = true;
            assert_eq!(pixel[3], 255, "alfa");
        }
        let distinct = values.iter().filter(|v| **v).count();
        println!(
            "klatka {index}: {}×{}, {:?}, różnych wartości kanałów: {distinct}",
            frame.width,
            frame.height,
            at.elapsed()
        );
        assert!(distinct > 1, "wszystkie piksele identyczne");
        if previous.as_ref().is_some_and(|p| *p != frame.pixels) {
            changed += 1;
        }
        sizes.push((frame.width, frame.height));
        previous = Some(frame.pixels);
    }
    assert!(
        sizes.windows(2).all(|w| w[0] == w[1]),
        "stały rozmiar klatek"
    );
    println!("klatek różnych od poprzedniej: {changed} z 4");
    let closing = Instant::now();
    drop(stream);
    println!("zamknięcie: {:?}", closing.elapsed());
    // Po zamknięciu kamerę da się otworzyć ponownie (źródło zostało zwolnione).
    let again = cameras.open(&first.id, 320).expect("ponowne otwarcie");
    assert!(again.size().0.max(again.size().1) <= 320);
}
