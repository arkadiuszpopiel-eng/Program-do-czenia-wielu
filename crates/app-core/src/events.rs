//! Strumień `alfa://events`: zdarzenia z modułów i komend grupowane w paczki najwyżej raz na
//! klatkę (PLAN §14.7 „batch co klatkę"). Brak zdarzeń = brak wybudzeń (idle ~0% CPU).
//! Sąsiednie `TextDelta` tej samej tury są scalane (tekst sklejony, bloki — ostatnia wersja
//! per indeks), a sąsiednie `MicLevel` — zastępowane najnowszym.

use std::sync::Arc;
use std::time::Duration;

use tokio::sync::{broadcast, mpsc};

use crate::dto::AlfaEvent;

/// Paczka zdarzeń wysyłana do UI jednym `emit`.
pub type EventBatch = Arc<Vec<AlfaEvent>>;

/// Domyślna długość klatki (≈ 60 Hz).
pub const DEFAULT_FRAME: Duration = Duration::from_millis(16);

/// Pojemność kanału paczek na subskrybenta (zaległy subskrybent traci najstarsze paczki).
const BATCH_CAPACITY: usize = 1024;

/// Nadawca zdarzeń (tani do klonowania).
#[derive(Clone)]
pub struct EventHub {
    tx: mpsc::UnboundedSender<AlfaEvent>,
    out: broadcast::Sender<EventBatch>,
}

impl EventHub {
    /// Uruchamia zadanie grupujące (wymaga środowiska tokio).
    pub fn start(frame: Duration) -> Self {
        let (tx, rx) = mpsc::unbounded_channel();
        let (out, _) = broadcast::channel(BATCH_CAPACITY);
        tokio::spawn(batcher(rx, out.clone(), frame));
        Self { tx, out }
    }

    /// Wysyła zdarzenie (trafi do najbliższej paczki).
    pub fn emit(&self, event: AlfaEvent) {
        // Zamknięty odbiornik = aplikacja się zamyka; zdarzenie nie ma już adresata.
        let _ = self.tx.send(event);
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
}

async fn batcher(
    mut rx: mpsc::UnboundedReceiver<AlfaEvent>,
    out: broadcast::Sender<EventBatch>,
    frame: Duration,
) {
    while let Some(first) = rx.recv().await {
        let mut batch = vec![first];
        if !frame.is_zero() {
            tokio::time::sleep(frame).await;
        }
        while let Ok(event) = rx.try_recv() {
            push_coalesced(&mut batch, event);
        }
        // Brak subskrybentów nie jest błędem (UI jeszcze nie nasłuchuje).
        let _ = out.send(Arc::new(batch));
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
            }),
            AlfaEvent::TextDelta {
                session_id: s2,
                turn_id: t2,
                text: more,
                blocks: newer,
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
        (Some(AlfaEvent::MicLevel { level }), AlfaEvent::MicLevel { level: newer }) => {
            *level = newer;
        }
        (_, event) => batch.push(event),
    }
}
