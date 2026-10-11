//! Strumień `alfa://events`: zdarzenia z modułów i komend grupowane w paczki najwyżej raz na
//! klatkę (PLAN §14.7 „batch co klatkę"). Brak zdarzeń = brak wybudzeń (idle ~0% CPU).
//! Sąsiednie `TextDelta` tej samej tury są scalane (tekst sklejony, bloki — ostatnia wersja
//! per indeks), a sąsiednie `MicLevel` i `CamerasChanged` — zastępowane najnowszym.
//! Paczka wychodzi jako ramka z numerami kolejnymi (`event_log.rs`, plan v6 zadanie B.2).

use std::sync::Arc;
use std::time::Duration;

use tokio::sync::{broadcast, mpsc};

use crate::dto::{AlfaEvent, EventsResume};
pub use crate::event_log::{EventBatch, EventLog};

/// Domyślna długość klatki (≈ 60 Hz).
pub const DEFAULT_FRAME: Duration = Duration::from_millis(16);

/// Pojemność kanału paczek na subskrybenta (zaległy subskrybent traci najstarsze paczki —
/// odzyskuje je przez [`EventHub::resume`]).
const BATCH_CAPACITY: usize = 1024;

/// Nadawca zdarzeń (tani do klonowania).
#[derive(Clone)]
pub struct EventHub {
    tx: mpsc::UnboundedSender<(u64, AlfaEvent)>,
    out: broadcast::Sender<EventBatch>,
    log: Arc<EventLog>,
}

impl EventHub {
    /// Uruchamia zadanie grupujące (wymaga środowiska tokio).
    pub fn start(frame: Duration) -> Self {
        let (tx, rx) = mpsc::unbounded_channel();
        let (out, _) = broadcast::channel(BATCH_CAPACITY);
        let log = Arc::new(EventLog::new());
        tokio::spawn(batcher(rx, out.clone(), log.clone(), frame));
        Self { tx, out, log }
    }

    /// Wysyła zdarzenie (trafi do najbliższej paczki); numer kolejny dostaje od razu.
    pub fn emit(&self, event: AlfaEvent) {
        // Zamknięty odbiornik = aplikacja się zamyka; zdarzenie nie ma już adresata.
        self.log.issue(|seq| {
            let _ = self.tx.send((seq, event));
        });
    }

    /// Wysyła kilka zdarzeń w kolejności.
    pub fn emit_all(&self, events: impl IntoIterator<Item = AlfaEvent>) {
        for event in events {
            self.emit(event);
        }
    }

    /// Nowy subskrybent paczek (powłoka Tauri: jeden; testy: dowolnie wiele).
    pub fn subscribe(&self) -> broadcast::Receiver<EventBatch> {
        self.out.subscribe()
    }

    /// Ramki po `after_seq` dla klienta, który zgubił paczki (polecenie `events_resume`).
    pub fn resume(&self, after_seq: u64, epoch: Option<&str>) -> EventsResume {
        self.log.resume(after_seq, epoch)
    }

    /// Znacznik uruchomienia i `seq` ostatniej wydanej ramki.
    pub fn position(&self) -> (&str, u64) {
        (self.log.epoch(), self.log.latest())
    }
}

async fn batcher(
    mut rx: mpsc::UnboundedReceiver<(u64, AlfaEvent)>,
    out: broadcast::Sender<EventBatch>,
    log: Arc<EventLog>,
    frame: Duration,
) {
    while let Some((mut seq, first)) = rx.recv().await {
        let mut batch = vec![first];
        if !frame.is_zero() {
            tokio::time::sleep(frame).await;
        }
        while let Ok((next, event)) = rx.try_recv() {
            seq = next;
            push_coalesced(&mut batch, event);
        }
        // Brak subskrybentów nie jest błędem (UI jeszcze nie nasłuchuje).
        let _ = out.send(log.seal(seq, batch));
    }
}

/// Dokłada zdarzenie do paczki, scalając je z poprzednim, gdy to bezpieczne.
pub fn push_coalesced(batch: &mut Vec<AlfaEvent>, event: AlfaEvent) {
    match (batch.last_mut(), event) {
        (
            Some(AlfaEvent::TextDelta {
                session_id,
                turn_id,
                text,
                blocks,
                // Scalona delta zaczyna się tam, gdzie pierwsza.
                offset: _,
            }),
            AlfaEvent::TextDelta {
                session_id: s2,
                turn_id: t2,
                text: more,
                blocks: newer,
                offset: _,
            },
        ) if *session_id == s2 && *turn_id == t2 => {
            text.push_str(&more);
            for block in newer {
                match blocks.iter_mut().find(|b| b.index == block.index) {
                    Some(existing) => *existing = block,
                    None => blocks.push(block),
                }
            }
            blocks.sort_by_key(|b| b.index);
        }
        (
            Some(AlfaEvent::MicLevel { level, raw }),
            AlfaEvent::MicLevel {
                level: newer,
                raw: newer_raw,
            },
        ) => {
            *level = newer;
            *raw = newer_raw;
        }
        // Poziom mowy: w paczce liczy się tylko najnowszy pomiar.
        (
            Some(AlfaEvent::SpeechLevel { agent, level }),
            AlfaEvent::SpeechLevel {
                agent: newer_agent,
                level: newer,
            },
        ) => {
            *agent = newer_agent;
            *level = newer;
        }
        // Stan kamer: pełna migawka — w paczce liczy się tylko najnowsza.
        (Some(AlfaEvent::CamerasChanged { state }), AlfaEvent::CamerasChanged { state: newer }) => {
            *state = newer;
        }
        (_, event) => batch.push(event),
    }
}
