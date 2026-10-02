//! Deterministyczny automat czytania (wspólny dla `-impl` i `-fake`): kolejka zdań, kursor,
//! tempo, sterowanie (pauza / wznów / dalej / wstecz / szybciej / wolniej / stop / od początku).
//! Polecenia dla usługi: mów zdanie `index` z tempem albo zatrzymaj wypowiedź `seq`.

use crate::segment::{Segment, segment};
use crate::{ReadAloudCfg, ReadAloudEvent, ReadControl, ReadPhase, ReadStatus, SourceText};

/// Polecenie automatu.
#[derive(Debug, Clone, PartialEq)]
pub enum ReadCommand {
    /// Synteza i odtworzenie zdania (`seq` — numer wypowiedzi automatu).
    Speak {
        /// Numer wypowiedzi.
        seq: u64,
        /// Indeks zdania.
        index: usize,
        /// Tekst zdania.
        text: String,
        /// Tempo (1,0 = naturalne).
        rate: f32,
    },
    /// Zatrzymanie wypowiedzi.
    Stop {
        /// Numer wypowiedzi.
        seq: u64,
    },
}

/// Automat czytania.
#[derive(Debug, Clone)]
pub struct ReadAloudMachine {
    cfg: ReadAloudCfg,
    segments: Vec<Segment>,
    index: usize,
    phase: ReadPhase,
    rate: f32,
    seq: u64,
    current: Option<u64>,
    source: Option<SourceText>,
    events: Vec<ReadAloudEvent>,
}

impl ReadAloudMachine {
    /// Automat z konfiguracją.
    pub fn new(cfg: ReadAloudCfg) -> Self {
        Self {
            rate: cfg.rate,
            cfg,
            segments: Vec::new(),
            index: 0,
            phase: ReadPhase::Idle,
            seq: 0,
            current: None,
            source: None,
            events: Vec::new(),
        }
    }

    /// Stan.
    pub fn status(&self) -> ReadStatus {
        ReadStatus {
            phase: self.phase,
            index: self.index,
            segments: self.segments.len(),
            rate: self.rate,
            highlight: self.segments.get(self.index).map(|s| (s.start, s.end)),
            app: self.source.as_ref().map(|s| s.app.clone()),
        }
    }

    /// Czytany tekst (niezaufany) — dla UI i udostępnienia za zgodą.
    pub fn source(&self) -> Option<&SourceText> {
        self.source.as_ref()
    }

    /// Odmowa odczytu (źródło) — zdarzenie bez treści.
    pub fn refuse(&mut self, reason: crate::RefuseReason) {
        self.events.push(ReadAloudEvent::Refused { reason });
    }

    /// Zdarzenia od ostatniego odczytu.
    pub fn take_events(&mut self) -> Vec<ReadAloudEvent> {
        std::mem::take(&mut self.events)
    }

    fn speak(&mut self) -> ReadCommand {
        self.seq += 1;
        self.current = Some(self.seq);
        self.phase = ReadPhase::Speaking;
        self.events.push(ReadAloudEvent::Segment {
            index: self.index as u32,
            of: self.segments.len() as u32,
        });
        ReadCommand::Speak {
            seq: self.seq,
            index: self.index,
            text: self
                .segments
                .get(self.index)
                .map(|s| s.text.clone())
                .unwrap_or_default(),
            rate: self.rate,
        }
    }

    fn stop_current(&mut self) -> Option<ReadCommand> {
        self.current.take().map(|seq| ReadCommand::Stop { seq })
    }

    /// Nowy tekst do czytania (poprzedni przerwany).
    pub fn load(&mut self, source: SourceText) -> Vec<ReadCommand> {
        let mut out: Vec<ReadCommand> = self.stop_current().into_iter().collect();
        self.segments = segment(source.text.as_untrusted_str(), self.cfg.max_segment_chars);
        self.index = 0;
        self.events.push(ReadAloudEvent::Started {
            app: source.app.clone(),
            scope: source.scope,
            segments: self.segments.len() as u32,
            chars: source.text.char_count() as u32,
            truncated: source.truncated,
        });
        self.source = Some(source);
        if self.segments.is_empty() {
            self.finish();
            return out;
        }
        out.push(self.speak());
        out
    }

    fn finish(&mut self) {
        self.phase = ReadPhase::Finished;
        self.current = None;
        self.events.push(ReadAloudEvent::Finished);
    }

    /// Sterowanie (UI, komenda głosowa, skrót).
    pub fn control(&mut self, c: ReadControl) -> Vec<ReadCommand> {
        if self.phase == ReadPhase::Idle {
            return Vec::new();
        }
        let mut out: Vec<ReadCommand> = Vec::new();
        match c {
            ReadControl::Pause if self.phase == ReadPhase::Speaking => {
                out.extend(self.stop_current());
                self.phase = ReadPhase::Paused;
                self.events.push(ReadAloudEvent::Paused);
            }
            ReadControl::Resume if self.phase == ReadPhase::Paused => {
                self.events.push(ReadAloudEvent::Resumed);
                out.push(self.speak());
            }
            ReadControl::Next | ReadControl::Previous | ReadControl::Restart => {
                out.extend(self.stop_current());
                self.index = match c {
                    ReadControl::Next => self.index + 1,
                    ReadControl::Previous => self.index.saturating_sub(1),
                    _ => 0,
                };
                if self.index >= self.segments.len() {
                    self.index = self.segments.len().saturating_sub(1);
                    self.finish();
                } else {
                    out.push(self.speak());
                }
            }
            ReadControl::Faster | ReadControl::Slower => {
                let step = if c == ReadControl::Faster {
                    self.cfg.rate_step
                } else {
                    -self.cfg.rate_step
                };
                let rate = ((self.rate + step) * 100.0).round() / 100.0;
                self.rate = rate.clamp(self.cfg.min_rate, self.cfg.max_rate);
                self.events.push(ReadAloudEvent::Rate {
                    permille: (self.rate * 1000.0).round() as u32,
                });
                // Nowe tempo od razu: bieżące zdanie od początku.
                if self.phase == ReadPhase::Speaking {
                    out.extend(self.stop_current());
                    out.push(self.speak());
                }
            }
            ReadControl::Stop => {
                out.extend(self.stop_current());
                self.clear();
                self.events.push(ReadAloudEvent::Stopped);
            }
            _ => {}
        }
        out
    }

    /// Wypowiedź `seq` skończyła się (dograna) — następne zdanie albo koniec.
    pub fn finished(&mut self, seq: u64) -> Vec<ReadCommand> {
        if self.current != Some(seq) || self.phase != ReadPhase::Speaking {
            return Vec::new();
        }
        self.current = None;
        self.index += 1;
        if self.index >= self.segments.len() {
            self.index = self.segments.len().saturating_sub(1);
            self.finish();
            return Vec::new();
        }
        vec![self.speak()]
    }

    /// Błąd syntezy wypowiedzi `seq` — czytanie kończy się zdarzeniem błędu.
    pub fn failed(&mut self, seq: u64, reason: &str) {
        if self.current == Some(seq) {
            self.current = None;
            self.events.push(ReadAloudEvent::Failed {
                reason: reason.to_owned(),
            });
            self.clear();
        }
    }

    /// Czyści treść z pamięci (stop, błąd).
    fn clear(&mut self) {
        self.segments.clear();
        self.source = None;
        self.index = 0;
        self.phase = ReadPhase::Idle;
    }
}
