//! Usługa na atrapach (`platform-fake` pulpit i schowek, `voice-tts-fake`, `voice-audio-fake`,
//! `scheduler-lite-fake`, wirtualny zegar): kontrakt (dokument, zaznaczenie przez zapas Ctrl+C,
//! odmowy), schowek przywracany po kopii, prywatność TTS, głośnik zajęty / odebrany.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use personas_contract::PersonaId;
use platform_contract::{
    ChordKey, ClipboardContent, ClipboardPort, GuiError, InputControl, InputPlan, InputPort,
    InputReport, InputStep, ScreenRect, WindowId,
};
use platform_fake::{FakeClipboard, FakeDesktop, FakeElement, FakeWindow};
use scheduler_lite_contract::{Holder, LeaseRequest, Priority, Resource, SchedulerLite};
use scheduler_lite_fake::FakeScheduler;
use voice_audio_contract::{AudioIo, StreamConfig};
use voice_audio_fake::FakeAudio;
use voice_readaloud_contract::contract_tests::{self, ReadDriver};
use voice_readaloud_contract::{
    ReadAloud, ReadAloudCfg, ReadAloudError, ReadPhase, ReadScope, RefuseReason, TextSource,
};
use voice_readaloud_impl::{CopyFallback, ReadAloudParts, ReadAloudService, UiaTextSource};
use voice_tts_fake::FakeTts;

/// Wejście: atrapa pulpitu + aplikacja kopiuje zaznaczenie do schowka na Ctrl+C.
struct CopyingInput {
    desk: Arc<FakeDesktop>,
    clipboard: Arc<FakeClipboard>,
    selections: Mutex<BTreeMap<u64, String>>,
}

impl InputPort for CopyingInput {
    fn send(&self, plan: &InputPlan, control: &InputControl) -> Result<InputReport, GuiError> {
        let report = self.desk.send(plan, control)?;
        let copy = plan.steps.iter().any(|s| {
            matches!(s, InputStep::Keys { chord } if chord.ctrl && chord.key == ChordKey::Letter('C'))
        });
        if copy && let Some(t) = self.selections.lock().unwrap().get(&plan.window.0) {
            self.clipboard
                .set(ClipboardContent::Text(t.clone()))
                .unwrap();
        }
        Ok(report)
    }
}

struct World {
    desk: Arc<FakeDesktop>,
    clipboard: Arc<FakeClipboard>,
    input: Arc<CopyingInput>,
    audio: FakeAudio,
    sched: Arc<FakeScheduler>,
    tts: Arc<FakeTts>,
}

fn rect() -> ScreenRect {
    ScreenRect::from_xywh(0, 0, 800, 600)
}

impl World {
    fn new() -> Arc<Self> {
        let desk = Arc::new(FakeDesktop::new());
        let clipboard = Arc::new(FakeClipboard::new());
        let input = Arc::new(CopyingInput {
            desk: desk.clone(),
            clipboard: clipboard.clone(),
            selections: Mutex::default(),
        });
        Arc::new(Self {
            desk,
            clipboard,
            input,
            audio: FakeAudio::new(),
            sched: Arc::new(FakeScheduler::new()),
            tts: Arc::new(FakeTts::new()),
        })
    }

    fn source(&self) -> UiaTextSource {
        UiaTextSource::new(self.desk.clone(), self.desk.clone()).with_copy_fallback(CopyFallback {
            input: self.input.clone(),
            clipboard: self.clipboard.clone(),
        })
    }

    fn service(&self) -> ReadAloudService {
        let parts = ReadAloudParts {
            source: Arc::new(self.source()),
            tts: self.tts.clone(),
            output: self
                .audio
                .open_output(None, &StreamConfig::output_default())
                .unwrap(),
            scheduler: self.sched.clone(),
        };
        ReadAloudService::new(parts, ReadAloudCfg::default())
    }
}

struct Driver(Arc<World>);

impl ReadDriver for Driver {
    fn open_document(&self, image: &str, text: &str) -> u64 {
        let w = self
            .0
            .desk
            .add_window(FakeWindow::new(image, image, rect()), true);
        self.0.desk.add_element(
            w,
            FakeElement::new("Dokument", "document", rect()).text(text),
        );
        w.0
    }
    fn open_password(&self) -> u64 {
        let w = self
            .0
            .desk
            .add_window(FakeWindow::new("Logowanie", "bank.exe", rect()), true);
        self.0.desk.add_element(
            w,
            FakeElement::new("Hasło", "edit", rect())
                .text("sekret")
                .password(),
        );
        w.0
    }
    fn select(&self, window: u64, text: &str) {
        self.0
            .input
            .selections
            .lock()
            .unwrap()
            .insert(window, text.to_owned());
    }
    fn advance(&self, ms: u64) {
        self.0.audio.advance(Duration::from_millis(ms));
        self.0.sched.advance(ms);
    }
}

#[tokio::test]
async fn contract_suite() {
    let w = World::new();
    let d = Driver(w.clone());
    contract_tests::reads_document_with_controls(&mut w.service(), &d).await;
    let w = World::new();
    let d = Driver(w.clone());
    contract_tests::reads_selection(&mut w.service(), &d).await;
    let w = World::new();
    let d = Driver(w.clone());
    contract_tests::refuses_protected_and_password(&mut w.service(), &d).await;
}

#[tokio::test]
async fn clipboard_is_restored_after_copy_fallback() {
    let w = World::new();
    let d = Driver(w.clone());
    w.clipboard
        .set(ClipboardContent::Text("moje dane".into()))
        .unwrap();
    let win = d.open_document("chrome.exe", "Strona. Długi tekst.");
    d.select(win, "Zaznaczony fragment.");
    let src = w.source().read(ReadScope::Selection, 1_000).unwrap();
    assert_eq!(src.text.as_untrusted_str(), "Zaznaczony fragment.");
    assert_eq!(src.app, "chrome.exe");
    assert_eq!(
        w.clipboard.get().unwrap(),
        ClipboardContent::Text("moje dane".into()),
        "schowek przywrócony"
    );
    let doc = w.source().read(ReadScope::Document, 8).unwrap();
    assert!(doc.truncated && doc.text.char_count() == 8);
    // Brak zaznaczenia (Ctrl+C nic nie kopiuje) — nie czytamy starej zawartości schowka.
    d.open_document("code.exe", "Kod.");
    assert_eq!(
        w.source().read(ReadScope::Selection, 100).err(),
        Some(ReadAloudError::Refused(RefuseReason::NoText))
    );
    assert_eq!(
        w.clipboard.get().unwrap(),
        ClipboardContent::Text("moje dane".into())
    );
    // Bez zapasu Ctrl+C i bez portu zaznaczenia — nieobsługiwane.
    let plain = UiaTextSource::new(w.desk.clone(), w.desk.clone());
    assert_eq!(
        plain.read(ReadScope::Selection, 100).err(),
        Some(ReadAloudError::Refused(RefuseReason::Unsupported))
    );
    assert!(WindowId(win).0 > 0);
}

#[tokio::test]
async fn busy_speaker_and_revoked_lease() {
    let w = World::new();
    let d = Driver(w.clone());
    d.open_document("notepad.exe", "Jedno. Dwa. Trzy.");
    let held = w
        .sched
        .acquire(LeaseRequest::new(
            Resource::Speaker,
            Holder::Persona(PersonaId::delta()),
            Priority::Critical,
            Duration::ZERO,
        ))
        .await
        .unwrap();
    let mut r = w.service();
    assert!(matches!(
        r.start(ReadScope::Document, PersonaId::beta()).await,
        Err(ReadAloudError::Platform(_))
    ));
    drop(held);
    w.sched.advance(10);
    r.start(ReadScope::Document, PersonaId::beta())
        .await
        .unwrap();
    assert_eq!(
        w.sched.holder(&Resource::Speaker).map(|l| l.holder),
        Some(Holder::Persona(PersonaId::beta()))
    );
    assert!(w.sched.kill_all() >= 1);
    let s = r.step().await;
    assert_eq!(s.phase, ReadPhase::Idle, "odebrany głośnik kończy czytanie");
}
