//! Atrapa `voice-readaloud`: automat z kontraktu + świat w pamięci ([`FakeReadWorld`]).

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

use std::sync::{Arc, Mutex, MutexGuard};

use async_trait::async_trait;
use personas_contract::PersonaId;
use platform_contract::{TargetGuard, image_file_name};
use voice_readaloud_contract::contract_tests::ReadDriver;
use voice_readaloud_contract::{
    ReadAloud, ReadAloudCfg, ReadAloudError, ReadAloudEvent, ReadAloudMachine, ReadCommand,
    ReadControl, ReadScope, ReadStatus, RefuseReason, ShareConsent, SourceText, TextSource,
    UntrustedText,
};

/// Czas „mówienia” znaku przy tempie 1,0 (ms).
pub const MS_PER_CHAR: f32 = 60.0;

#[derive(Debug, Clone)]
struct Win {
    id: u64,
    image: String,
    text: String,
    selection: Option<String>,
    password: bool,
}

#[derive(Debug, Default)]
struct World {
    windows: Vec<Win>,
    focus: Option<u64>,
    now_ms: u64,
}

/// Świat atrapy: okna, fokus, zegar.
#[derive(Debug, Clone, Default)]
pub struct FakeReadWorld(Arc<Mutex<World>>);

impl FakeReadWorld {
    fn lock(&self) -> MutexGuard<'_, World> {
        self.0.lock().unwrap_or_else(|p| p.into_inner())
    }

    fn add(&self, image: &str, text: &str, password: bool) -> u64 {
        let mut w = self.lock();
        let id = w.windows.len() as u64 + 1;
        w.windows.push(Win {
            id,
            image: image.into(),
            text: text.into(),
            selection: None,
            password,
        });
        w.focus = Some(id);
        id
    }

    /// Czas wirtualny (ms).
    pub fn now_ms(&self) -> u64 {
        self.lock().now_ms
    }
}

impl ReadDriver for FakeReadWorld {
    fn open_document(&self, image: &str, text: &str) -> u64 {
        self.add(image, text, false)
    }
    fn open_password(&self) -> u64 {
        self.add("bank.exe", "sekret", true)
    }
    fn select(&self, window: u64, text: &str) {
        if let Some(w) = self.lock().windows.iter_mut().find(|w| w.id == window) {
            w.selection = Some(text.into());
        }
    }
    fn advance(&self, ms: u64) {
        self.lock().now_ms += ms;
    }
}

impl TextSource for FakeReadWorld {
    fn read(&self, scope: ReadScope, max_chars: usize) -> Result<SourceText, ReadAloudError> {
        let g = self.lock();
        let w = g
            .focus
            .and_then(|f| g.windows.iter().find(|w| w.id == f))
            .ok_or(ReadAloudError::Refused(RefuseReason::NoForeground))?;
        if TargetGuard::baseline().is_protected(u32::MAX, &w.image) {
            return Err(ReadAloudError::Refused(RefuseReason::ProtectedTarget));
        }
        if w.password {
            return Err(ReadAloudError::Refused(RefuseReason::PasswordField));
        }
        let text = match scope {
            ReadScope::Document => w.text.clone(),
            ReadScope::Selection => w
                .selection
                .clone()
                .ok_or(ReadAloudError::Refused(RefuseReason::NoText))?,
        };
        Ok(SourceText {
            truncated: text.chars().count() > max_chars,
            text: UntrustedText::new(text.chars().take(max_chars).collect::<String>()),
            app: image_file_name(&w.image),
            scope,
        })
    }
}

/// Atrapa czytania.
#[derive(Debug)]
pub struct FakeReadAloud {
    world: FakeReadWorld,
    machine: ReadAloudMachine,
    cfg: ReadAloudCfg,
    /// (seq, koniec mówienia w ms).
    playing: Option<(u64, u64)>,
}

impl FakeReadAloud {
    /// Atrapa na świecie.
    pub fn new(world: FakeReadWorld) -> Self {
        let cfg = ReadAloudCfg::default();
        Self {
            world,
            machine: ReadAloudMachine::new(cfg),
            cfg,
            playing: None,
        }
    }

    fn apply(&mut self, commands: Vec<ReadCommand>) {
        for c in commands {
            match c {
                ReadCommand::Stop { seq } => {
                    self.playing = self.playing.filter(|(s, _)| *s != seq);
                }
                ReadCommand::Speak {
                    seq, text, rate, ..
                } => {
                    let dur = (text.chars().count() as f32 * MS_PER_CHAR / rate.max(0.1)) as u64;
                    self.playing = Some((seq, self.world.now_ms() + dur.max(1)));
                }
            }
        }
    }
}

#[async_trait]
impl ReadAloud for FakeReadAloud {
    async fn start(
        &mut self,
        scope: ReadScope,
        _persona: PersonaId,
    ) -> Result<ReadStatus, ReadAloudError> {
        let text = self
            .world
            .read(scope, self.cfg.max_chars)
            .inspect_err(|e| {
                if let ReadAloudError::Refused(r) = e {
                    self.machine.refuse(*r);
                }
            })?;
        let c = self.machine.load(text);
        self.apply(c);
        Ok(self.machine.status())
    }

    async fn step(&mut self) -> ReadStatus {
        if let Some((seq, end)) = self.playing
            && self.world.now_ms() >= end
        {
            self.playing = None;
            let c = self.machine.finished(seq);
            self.apply(c);
        }
        self.machine.status()
    }

    async fn control(&mut self, control: ReadControl) -> ReadStatus {
        let c = self.machine.control(control);
        self.apply(c);
        self.machine.status()
    }

    fn status(&self) -> ReadStatus {
        self.machine.status()
    }

    fn share_with_model(&self, consent: Option<&ShareConsent>) -> Result<String, ReadAloudError> {
        let s = self.machine.source().ok_or(ReadAloudError::NotReading)?;
        s.text
            .for_model(consent, &s.app)
            .ok_or(ReadAloudError::ConsentRequired)
    }

    fn take_events(&mut self) -> Vec<ReadAloudEvent> {
        self.machine.take_events()
    }
}
