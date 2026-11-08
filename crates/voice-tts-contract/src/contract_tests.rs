//! Współdzielone testy kontraktowe `Tts` (feature `contract-tests`).

use personas_contract::PersonaId;
use providers_contract::PrivacyTag;

use crate::{CancelToken, SpeechStyle, TTS_RATE, Tts, TtsChunk, TtsError, TtsEvent, TtsRequest};

/// Żądanie testowe.
pub fn request(utterance: u64, persona: PersonaId, text: &str) -> TtsRequest {
    TtsRequest {
        utterance,
        persona,
        text: text.into(),
        style: SpeechStyle::default(),
        cacheable: false,
        privacy: PrivacyTag::Normal,
    }
}

/// Odbiera cały strumień.
pub async fn collect<T: Tts>(
    tts: &T,
    req: TtsRequest,
    cancel: CancelToken,
) -> Result<Vec<TtsChunk>, TtsError> {
    let mut rx = tts.synth(req, cancel).await?;
    let mut out = Vec::new();
    while let Some(c) = rx.recv().await {
        let c = c?;
        let last = c.is_last;
        out.push(c);
        if last {
            break;
        }
    }
    Ok(out)
}

/// Dwa zdania → co najmniej dwa fragmenty (zdanie po zdaniu), znaczniki wszystkich słów w kolejności
/// i w czasie audio, ostatni oznaczony, zdarzenia `started` (TTFB) i `finished`.
pub async fn streams_sentences_with_word_marks<T: Tts>(tts: &T) {
    let text = "Dzień dobry, tu Alfa. Sprawdziłam pocztę i mam trzy nowe wiadomości!";
    let chunks = collect(tts, request(1, PersonaId::alfa(), text), CancelToken::new())
        .await
        .unwrap_or_else(|e| panic!("{e}"));
    assert!(chunks.len() >= 2, "fragmentów: {}", chunks.len());
    assert!(chunks.last().is_some_and(|c| c.is_last));
    assert!(
        chunks
            .iter()
            .enumerate()
            .all(|(i, c)| c.seq == i as u32 && c.audio.format.sample_rate == TTS_RATE)
    );
    let marks: Vec<_> = chunks.iter().flat_map(|c| c.marks.clone()).collect();
    let words: Vec<&str> = text.split_whitespace().collect();
    assert_eq!(marks.len(), words.len());
    assert!(
        marks
            .iter()
            .enumerate()
            .all(|(i, m)| m.word_idx == i as u32 && m.word == words[i])
    );
    assert!(
        marks
            .windows(2)
            .all(|w| w[0].start_ms <= w[1].start_ms && w[0].end_ms <= w[1].start_ms + 1)
    );
    let total_ms: u32 = chunks
        .iter()
        .map(|c| c.audio.duration().as_millis() as u32)
        .sum();
    assert!(
        marks.last().is_some_and(|m| m.end_ms <= total_ms + 1),
        "znaczniki w czasie audio"
    );
    let events = tts.take_events();
    assert!(
        events
            .iter()
            .any(|e| matches!(e, TtsEvent::Started { utterance: 1, .. }))
    );
    assert!(
        events
            .iter()
            .any(|e| matches!(e, TtsEvent::Finished { utterance: 1, .. }))
    );
}

/// Anulowanie przed startem / `stop` kończy strumień bez dalszych fragmentów.
pub async fn stop_cancels<T: Tts>(tts: &T) {
    let cancel = CancelToken::new();
    cancel.cancel();
    let r = collect(
        tts,
        request(
            2,
            PersonaId::beta(),
            "Pierwsze zdanie. Drugie zdanie. Trzecie.",
        ),
        cancel,
    )
    .await;
    assert!(matches!(r, Err(TtsError::Cancelled)) || r.is_ok_and(|c| c.is_empty()));
    tts.stop(2);
    assert!(
        tts.take_events()
            .iter()
            .any(|e| matches!(e, TtsEvent::Stopped { utterance: 2 }))
    );
}

/// Pusty tekst i agentka bez głosu są błędami.
pub async fn rejects_bad_requests<T: Tts>(tts: &T) {
    assert!(matches!(
        tts.synth(request(3, PersonaId::gama(), "  "), CancelToken::new())
            .await,
        Err(TtsError::EmptyText)
    ));
    assert!(matches!(
        tts.synth(
            request(4, PersonaId::new("nikt"), "Hej"),
            CancelToken::new()
        )
        .await,
        Err(TtsError::NoVoice(_))
    ));
    assert_eq!(tts.voices().len(), 4, "cztery głosy v0");
}

/// Cały zestaw.
pub async fn run_all<T: Tts>(tts: &T) {
    streams_sentences_with_word_marks(tts).await;
    stop_cancels(tts).await;
    rejects_bad_requests(tts).await;
}
