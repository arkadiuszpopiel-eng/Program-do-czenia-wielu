//! Atrapy `tools-vision` (docs/modules/tools-vision/SPEC.md, sekcja „Fake”):
//! - [`FakeTools`] — prawdziwe manifesty i walidacja argumentów z kontraktu, wyniki skryptowane
//!   (FIFO), odczyty oznaczone jako niezaufane; bez Brokera i bez we/wy (runtime, ewaluacje, UI);
//! - [`FakeOcr`] — deterministyczny `OcrPort`: wynik skryptowany albo stały tekst, zapis obrazów
//!   (testy maskowania „przed OCR”), języki `pl` i `en-US`;
//! - [`FakeDescriber`] — `DescribePort` z regułą prywatności (sesja „tylko lokalnie” bez modelu
//!   lokalnego → odmowa), zapis żądań;
//! - [`FakePrivacy`] — mapa sesja → prywatność (nieznana sesja → „tylko lokalnie”).

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

use std::collections::{BTreeMap, VecDeque};
use std::sync::{Arc, Mutex, MutexGuard};

use async_trait::async_trait;
use providers_contract::CancellationToken;
use tools_common_contract::{
    RecordedCall, Tool, ToolCtx, ToolErrorKind, ToolManifest, ToolOutcome, Toolset,
};
use tools_vision_contract::{
    DescribeError, DescribePort, DescribeRequest, Description, OcrError, OcrLine, OcrPort, OcrRect,
    OcrRequest, OcrText, OcrWord, PrivacyLookup, VisionPrivacy, check_args, manifests,
};

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|p| p.into_inner())
}

#[derive(Debug, Default)]
struct State {
    queue: VecDeque<(String, ToolOutcome)>,
    calls: Vec<(String, RecordedCall)>,
}

/// Atrapa zestawu narzędzi.
#[derive(Debug, Clone, Default)]
pub struct FakeTools {
    state: Arc<Mutex<State>>,
}

impl FakeTools {
    /// Wynik najbliższego wywołania narzędzia `tool` (FIFO).
    pub fn push(&self, tool: &str, outcome: ToolOutcome) {
        lock(&self.state)
            .queue
            .push_back((tool.to_owned(), outcome));
    }

    /// Wywołania (narzędzie, zapis).
    pub fn calls(&self) -> Vec<(String, RecordedCall)> {
        lock(&self.state).calls.clone()
    }
}

struct FakeTool {
    owner: FakeTools,
    manifest: ToolManifest,
}

#[async_trait]
impl Tool for FakeTool {
    fn manifest(&self) -> &ToolManifest {
        &self.manifest
    }

    async fn call(&self, args: serde_json::Value, ctx: &ToolCtx) -> ToolOutcome {
        let name = self.manifest.name.clone();
        if !args.is_object() {
            return ToolOutcome::failed(
                ToolErrorKind::InvalidArgs,
                "Niepoprawne argumenty: oczekiwano obiektu JSON.",
            );
        }
        if let Err(e) = check_args(&name, &args) {
            return ToolOutcome::failed(
                ToolErrorKind::InvalidArgs,
                format!("Niepoprawne argumenty: {e}."),
            );
        }
        if ctx.cancel.is_cancelled() {
            return ToolOutcome::cancelled(&self.manifest.title);
        }
        let mut s = lock(&self.owner.state);
        s.calls.push((
            name.clone(),
            RecordedCall {
                args,
                holder: ctx.holder.clone(),
                step: ctx.step,
                untrusted_args: ctx.untrusted_args,
            },
        ));
        let scripted = s
            .queue
            .iter()
            .position(|(t, _)| *t == name)
            .and_then(|i| s.queue.remove(i))
            .map(|(_, o)| o);
        let mut out = scripted.unwrap_or_else(|| {
            ToolOutcome::ok(
                format!("{}: wykonano (atrapa).", self.manifest.title),
                serde_json::json!({}),
            )
        });
        if out.is_ok() && out.untrusted.is_none() {
            out.untrusted = self.manifest.untrusted_output.clone();
        }
        out
    }
}

impl Toolset for FakeTools {
    fn tools(&self) -> Vec<Arc<dyn Tool>> {
        manifests()
            .into_iter()
            .map(|manifest| {
                Arc::new(FakeTool {
                    owner: self.clone(),
                    manifest,
                }) as Arc<dyn Tool>
            })
            .collect()
    }
}

/// Linia tekstu atrapy OCR (słowa rozłożone równo w prostokącie linii).
pub fn line(text: &str, x: f32, y: f32, width: f32, height: f32) -> OcrLine {
    let words: Vec<&str> = text.split_whitespace().collect();
    let step = width / words.len().max(1) as f32;
    OcrLine {
        text: text.to_owned(),
        words: words
            .iter()
            .enumerate()
            .map(|(i, w)| OcrWord {
                text: (*w).to_owned(),
                rect: OcrRect {
                    x: x + step * i as f32,
                    y,
                    width: step * 0.9,
                    height,
                },
            })
            .collect(),
    }
}

#[derive(Default)]
struct OcrState {
    scripted: VecDeque<Result<OcrText, OcrError>>,
    requests: Vec<OcrRequest>,
}

/// Deterministyczny port OCR.
#[derive(Clone)]
pub struct FakeOcr {
    state: Arc<Mutex<OcrState>>,
    lines: Vec<OcrLine>,
    languages: Vec<String>,
}

impl Default for FakeOcr {
    fn default() -> Self {
        Self::new(vec![line("Atrapa OCR", 0.0, 0.0, 100.0, 12.0)])
    }
}

impl FakeOcr {
    /// Port zwracający zawsze te same linie (języki `pl`, `en-US`).
    pub fn new(lines: Vec<OcrLine>) -> Self {
        Self {
            state: Arc::default(),
            lines,
            languages: vec!["pl".into(), "en-US".into()],
        }
    }

    /// Wynik najbliższego wywołania (FIFO), przed stałym tekstem.
    pub fn push(&self, result: Result<OcrText, OcrError>) {
        lock(&self.state).scripted.push_back(result);
    }

    /// Żądania (obrazy i języki) w kolejności.
    pub fn requests(&self) -> Vec<OcrRequest> {
        lock(&self.state).requests.clone()
    }
}

impl OcrPort for FakeOcr {
    fn recognize(&self, request: &OcrRequest) -> Result<OcrText, OcrError> {
        let mut s = lock(&self.state);
        s.requests.push(request.clone());
        if let Some(r) = s.scripted.pop_front() {
            return r;
        }
        if request.image.is_empty() {
            return Err(OcrError::Image("pusty obraz".into()));
        }
        let language = request.language.clone().unwrap_or_else(|| "pl".into());
        if !self
            .languages
            .iter()
            .any(|l| l.eq_ignore_ascii_case(&language))
        {
            return Err(OcrError::Language(language));
        }
        Ok(OcrText {
            language,
            lines: self.lines.clone(),
            angle: None,
        })
    }

    fn languages(&self) -> Result<Vec<String>, OcrError> {
        Ok(self.languages.clone())
    }
}

#[derive(Default)]
struct DescribeState {
    scripted: VecDeque<Result<String, DescribeError>>,
    requests: Vec<DescribeRequest>,
}

/// Port opisu obrazu z regułą prywatności.
#[derive(Clone)]
pub struct FakeDescriber {
    state: Arc<Mutex<DescribeState>>,
    local: bool,
    cloud: bool,
}

impl FakeDescriber {
    /// `local` — jest lokalny model z wizją; `cloud` — jest model chmurowy z wizją.
    pub fn new(local: bool, cloud: bool) -> Self {
        Self {
            state: Arc::default(),
            local,
            cloud,
        }
    }

    /// Tekst (albo błąd) najbliższego opisu (FIFO).
    pub fn push(&self, result: Result<String, DescribeError>) {
        lock(&self.state).scripted.push_back(result);
    }

    /// Żądania w kolejności.
    pub fn requests(&self) -> Vec<DescribeRequest> {
        lock(&self.state).requests.clone()
    }
}

#[async_trait]
impl DescribePort for FakeDescriber {
    async fn describe(
        &self,
        request: DescribeRequest,
        cancel: CancellationToken,
    ) -> Result<Description, DescribeError> {
        if cancel.is_cancelled() {
            return Err(DescribeError::Cancelled);
        }
        let local = match request.privacy {
            VisionPrivacy::LocalOnly if !self.local => return Err(DescribeError::PrivateNoLocal),
            VisionPrivacy::LocalOnly => true,
            VisionPrivacy::Normal if self.cloud => false,
            VisionPrivacy::Normal if self.local => true,
            VisionPrivacy::Normal => {
                return Err(DescribeError::NoVisionModel("brak modeli w atrapie".into()));
            }
        };
        let mut s = lock(&self.state);
        let text = match s.scripted.pop_front() {
            Some(r) => r?,
            None => format!(
                "Atrapa: obraz {} ({} B).",
                request.media_type,
                request.image.len()
            ),
        };
        s.requests.push(request);
        Ok(Description {
            text,
            model: if local {
                "local:fake-vision"
            } else {
                "fake:vision"
            }
            .into(),
            local,
        })
    }
}

/// Mapa prywatności sesji (nieznana sesja → „tylko lokalnie”).
#[derive(Debug, Clone, Default)]
pub struct FakePrivacy {
    sessions: Arc<Mutex<BTreeMap<String, VisionPrivacy>>>,
}

impl FakePrivacy {
    /// Ustawia prywatność sesji.
    pub fn set(&self, session: &str, privacy: VisionPrivacy) {
        lock(&self.sessions).insert(session.to_owned(), privacy);
    }
}

impl PrivacyLookup for FakePrivacy {
    fn vision_privacy(&self, session: &str) -> VisionPrivacy {
        lock(&self.sessions)
            .get(session)
            .copied()
            .unwrap_or_default()
    }
}
