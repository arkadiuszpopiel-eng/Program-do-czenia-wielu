//! Miernik mowy (`speech_meter`): obwiednia fragmentu, poziomy w rytm odtwarzania, znacznik
//! czytanego zdania w chwili, gdy zdanie zaczyna brzmieć (plan 43), cisza po „Stop”.
//! Testy poza `src` (limit rozmiaru crate'a).

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::sync::{Arc, Mutex};
use std::time::Duration;

use app_api::dto::AlfaEvent;
use app_modules::speech_meter::{SpeechMeter, envelope, sentence_segment};
use voice_audio_contract::{AudioFormat, Frame, MediaTime};
use voice_tts_contract::split_sentences;

fn tone(seconds: f32, amplitude: f32) -> Vec<f32> {
    let n = (24_000.0 * seconds) as usize;
    (0..n)
        .map(|i| amplitude * (i as f32 * 0.05).sin())
        .collect()
}

#[test]
fn envelope_follows_loudness_window_by_window() {
    let format = AudioFormat::mono(24_000);
    let mut pcm = tone(0.34, 0.5);
    pcm.extend(vec![0.0; 24_000 * 34 / 100]);
    pcm.extend(tone(0.34, 0.05));
    let levels = envelope(&pcm, format);
    assert_eq!(levels.len(), 30);
    let (loud, silent, quiet) = (levels[3], levels[14], levels[25]);
    assert!(loud > 0.8, "{loud}");
    assert_eq!(silent, 0.0);
    assert!(quiet > 0.3 && quiet < loud, "{quiet}");
    assert!(envelope(&[], format).is_empty());
}

#[tokio::test(start_paused = true)]
async fn chunks_play_back_to_back_and_stop_silences_the_rest() {
    let seen: Arc<Mutex<Vec<(String, f64)>>> = Arc::default();
    let store = seen.clone();
    let meter = SpeechMeter::with_sink(Arc::new(move |event| {
        if let AlfaEvent::SpeechLevel { agent, level } = event {
            store.lock().expect("zamek").push((agent, level));
        }
    }));
    let take = || std::mem::take(&mut *seen.lock().expect("zamek"));
    let frame = |amplitude: f32| {
        Frame::new(
            tone(0.17, amplitude),
            AudioFormat::mono(24_000),
            MediaTime(0),
        )
        .expect("ramka")
    };
    // Dwa fragmenty po 170 ms (5 okien): drugi zaczyna się po pierwszym.
    meter.schedule("alfa", &frame(0.5));
    meter.schedule("alfa", &frame(0.05));
    tokio::time::sleep(Duration::from_millis(400)).await;
    let first = take();
    assert!(first.iter().all(|(agent, _)| agent == "alfa"));
    let levels: Vec<f64> = first.into_iter().map(|(_, level)| level).collect();
    // 5 głośnych, 5 cichych bez przerwy między fragmentami i zero na końcu.
    assert_eq!(levels.len(), 11, "{levels:?}");
    assert!(levels[..5].iter().all(|l| *l > 0.8));
    assert!(levels[5..10].iter().all(|l| *l > 0.2 && *l < 0.7));
    assert_eq!(levels[10], 0.0);

    // Stop w trakcie: po ciszy nie przychodzi już nic z przerwanego fragmentu.
    meter.schedule("beta", &frame(0.5));
    tokio::time::sleep(Duration::from_millis(40)).await;
    meter.silence();
    tokio::time::sleep(Duration::from_millis(400)).await;
    let after: Vec<f64> = take().into_iter().map(|(_, level)| level).collect();
    assert_eq!(after.last(), Some(&0.0));
    assert!(after.len() <= 3, "{after:?}");
}

#[tokio::test(start_paused = true)]
async fn marker_goes_out_when_its_chunk_starts_and_stop_drops_it() {
    let seen: Arc<Mutex<Vec<String>>> = Arc::default();
    let store = seen.clone();
    let meter = SpeechMeter::with_sink(Arc::new(move |event| {
        let label = match event {
            AlfaEvent::SpeechSegment { index, .. } => format!("zdanie {index}"),
            AlfaEvent::SpeechLevel { level, .. } if level > 0.0 => "poziom".to_owned(),
            AlfaEvent::SpeechLevel { .. } => "cisza".to_owned(),
            _ => "inne".to_owned(),
        };
        store.lock().expect("zamek").push(label);
    }));
    let take = || std::mem::take(&mut *seen.lock().expect("zamek"));
    let frame =
        || Frame::new(tone(0.17, 0.5), AudioFormat::mono(24_000), MediaTime(0)).expect("ramka");
    let segment = |index: u32| AlfaEvent::SpeechSegment {
        turn_id: "s/1".into(),
        index,
        of: 2,
        text: format!("Zdanie {index}."),
    };
    // Dwa zdania po 170 ms (5 okien): znacznik drugiego wychodzi dopiero po pięciu poziomach
    // pierwszego — wyróżnienie nie wyprzedza głosu.
    meter.mark_next(segment(0));
    meter.schedule("alfa", &frame());
    meter.mark_next(segment(1));
    meter.schedule("alfa", &frame());
    tokio::time::sleep(Duration::from_millis(400)).await;
    let all = take();
    assert_eq!(all.first().map(String::as_str), Some("zdanie 0"), "{all:?}");
    let second = all
        .iter()
        .position(|e| e == "zdanie 1")
        .expect("drugie zdanie");
    assert_eq!(second, 6, "{all:?}");
    assert_eq!(all.last().map(String::as_str), Some("cisza"), "{all:?}");

    // „Stop” przed zabrzmieniem: znacznik przepada razem z poziomami.
    meter.mark_next(segment(0));
    meter.silence();
    meter.schedule("alfa", &frame());
    tokio::time::sleep(Duration::from_millis(300)).await;
    let after = take();
    assert!(!after.iter().any(|e| e.starts_with("zdanie")), "{after:?}");
}

#[test]
fn segment_carries_sentence_and_end_has_empty_text() {
    let all = split_sentences("Ala ma kota. Kot ma Alę!");
    let AlfaEvent::SpeechSegment {
        turn_id,
        index,
        of,
        text,
    } = sentence_segment("s/3", &all, 1)
    else {
        panic!("zły rodzaj zdarzenia");
    };
    assert_eq!(
        (turn_id.as_str(), index, of, text.as_str()),
        ("s/3", 1, 2, "Kot ma Alę!")
    );
    let AlfaEvent::SpeechSegment {
        index, of, text, ..
    } = sentence_segment("s/3", &all, of)
    else {
        panic!("zły rodzaj zdarzenia");
    };
    assert_eq!((index, of, text.as_str()), (2, 2, ""));
}
