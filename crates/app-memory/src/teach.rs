//! „Naucz Alfę” (P5): wiedza z wklejonego tekstu, pliku (tekst wyciągnięty przez `app-files`)
//! albo bieżącej rozmowy trafia do pamięci F7 jako fakty semantyczne ze źródłem.
//!
//! Zaufanie i zakres:
//! - **tekst i plik** to treść z zewnątrz — jak załączniki: `Provenance::UntrustedContent`
//!   ze źródłem `naucz: … · sha256:…` (dane, nie polecenia; w kontekście czatu oznaczone
//!   „[niezaufane]”). Kontrakt pamięci wpuszcza treść niezaufaną wyłącznie do zakresu sesji
//!   (bez awansu), więc inny zakres → odmowa z wyjaśnieniem;
//! - **rozmowa** (aktywna gałąź bez ukrytych tur, narzędzi i komunikatów systemowych):
//!   tury użytkownika — jak „Zapamiętaj” na turze: `Provenance::User` z pochodzeniem z tury
//!   (sesja prywatna nie zasila zakresów szerszych — reguła kontraktu). Odpowiedzi agentek
//!   często powtarzają treść z zewnątrz (strony, wyniki narzędzi, załączniki), a nauka zapisuje
//!   je hurtem, bez przeczytania przez właściciela — dlatego trafiają **tylko do zakresu sesji**
//!   jako `Provenance::UntrustedContent` (źródło `rozmowa:… · odpowiedź agentki`); przy zakresie
//!   projektu albo globalnym nauka bierze wyłącznie tury użytkownika.
//!
//! Każdy fakt: `[tytuł · i/n] fragment`, encja = tytuł, zapis jawny właściciela (od razu
//! aktywny). Błąd w połowie → zapisane fakty są zapominane (wszystko albo nic). Cofnięcie:
//! `memory_forget` na identyfikatorach z raportu (UI) albo Inspektor.

use app_api::AppError;
use app_api::dto::{RememberScope, TeachReport, TeachRequest, TeachSource};
use memory_contract::{
    Accessor, EntryRef, ForgetTarget, Layer, MemoryScope, NewMemory, Origin, Provenance,
    RememberMode, SessionId, scope_key,
};
use sessions_contract::{Role, SessionHistory};
use sha2::{Digest, Sha256};

use crate::app::MemoryApp;
use crate::chunk::{Chunks, Limits, chunk, one_line, paragraphs, take_chars};
use crate::ids::entry_dto;

/// Najwięcej znaków tytułu (encja i przedrostek faktu).
pub const TITLE_CHARS: usize = 80;
/// Pewność faktów z treści niezaufanej.
pub const UNTRUSTED_CONFIDENCE: f32 = 0.7;
/// Najwięcej bajtów tekstu w żądaniu (wklejony tekst, plik) — więcej → odmowa bez przetwarzania.
pub const MAX_REQUEST_BYTES: usize = 4 * 1024 * 1024;
/// Zapas znaków ponad limit wejścia przy wstępnym ucięciu surowego tekstu (białe znaki
/// zwijane przy podziale).
const RAW_MARGIN_CHARS: usize = 20_000;

/// Plan nauki: źródło, zaufanie i fragmenty.
#[derive(Debug, Clone, PartialEq)]
pub struct TeachPlan {
    /// Tytuł (przedrostek faktów, encja).
    pub title: String,
    /// Pochodzenie zapisywane przy faktach.
    pub provenance: Provenance,
    /// Opis źródła w raporcie.
    pub source: String,
    /// Fragmenty.
    pub chunks: Chunks,
    /// Tury agentek (rozmowa, tylko zakres sesji) — ich fragmenty zapisywane jako niezaufane.
    pub agent_turns: Vec<u64>,
}

impl TeachPlan {
    /// Czy cała treść jest zaufana (rozmowa bez odpowiedzi agentek).
    pub fn trusted(&self) -> bool {
        (0..self.chunks.pieces.len()).all(|i| self.provenance_of(i).is_trusted())
            && self.provenance.is_trusted()
    }

    /// Pochodzenie faktu `i`: fragment z tury agentki → treść niezaufana.
    pub fn provenance_of(&self, i: usize) -> Provenance {
        let agent = self
            .chunks
            .pieces
            .get(i)
            .and_then(|p| p.turn)
            .is_some_and(|t| self.agent_turns.contains(&t));
        if agent {
            Provenance::UntrustedContent {
                source: format!("{} · odpowiedź agentki", self.source),
            }
        } else {
            self.provenance.clone()
        }
    }

    /// Treść faktu `i` (od zera): `[tytuł · i/n] fragment`.
    pub fn fact_text(&self, i: usize) -> Option<String> {
        let piece = self.chunks.pieces.get(i)?;
        let n = self.chunks.pieces.len();
        Some(if n == 1 {
            format!("[{}] {}", self.title, piece.text)
        } else {
            format!("[{} · {}/{n}] {}", self.title, i + 1, piece.text)
        })
    }
}

/// Tytuł: jedna linia bez nawiasów kwadratowych, najwyżej [`TITLE_CHARS`] znaków.
pub fn clean_title(title: Option<&str>, fallback: &str) -> String {
    let pick = |s: &str| {
        let line = one_line(s).replace(['[', ']'], "");
        take_chars(line.trim(), TITLE_CHARS).trim().to_owned()
    };
    let title = title.map(pick).unwrap_or_default();
    if title.is_empty() {
        pick(fallback)
    } else {
        title
    }
}

/// Skrót treści (16 znaków szesnastkowych SHA-256) — rozpoznanie tego samego źródła.
pub fn content_hash(text: &str) -> String {
    let digest = Sha256::digest(text.as_bytes());
    digest.iter().take(8).map(|b| format!("{b:02x}")).collect()
}

/// Plan dla tekstu albo pliku (treść niezaufana).
pub fn external_plan(source: &TeachSource, title: Option<&str>, limits: Limits) -> TeachPlan {
    let (text, label, fallback) = match source {
        TeachSource::File { name, text } => {
            let name = clean_title(Some(name), "plik");
            (text.as_str(), format!("plik „{name}”"), name)
        }
        TeachSource::Text { text } => (
            text.as_str(),
            "wklejony tekst".to_owned(),
            "Wklejony tekst".to_owned(),
        ),
        TeachSource::Conversation => ("", "rozmowa".to_owned(), "Rozmowa".to_owned()),
    };
    // Wstępne ucięcie surowego tekstu: podział i skrót nie przechodzą całego, bardzo długiego
    // wejścia, skoro i tak liczy się tylko początek (limit znaków wejścia).
    let raw = take_chars(
        text,
        limits.max_input_chars.saturating_add(RAW_MARGIN_CHARS),
    );
    let cut = raw.len() < text.len();
    let segments: Vec<(Option<u64>, String)> =
        paragraphs(raw).into_iter().map(|p| (None, p)).collect();
    let source = format!("naucz: {label} · sha256:{}", content_hash(raw));
    let mut chunks = chunk(&segments, limits);
    chunks.truncated |= cut;
    TeachPlan {
        title: clean_title(title, &fallback),
        provenance: Provenance::UntrustedContent {
            source: source.clone(),
        },
        source,
        chunks,
        agent_turns: Vec::new(),
    }
}

/// Plan dla rozmowy: aktywna gałąź bez tur ukrytych, narzędzi i systemowych. Odpowiedzi
/// agentek tylko przy `with_agents` (zakres sesji) — jako treść niezaufana.
pub fn conversation_plan(
    history: &dyn SessionHistory,
    session: &SessionId,
    title: String,
    limits: Limits,
    with_agents: bool,
) -> Result<TeachPlan, AppError> {
    let leaf = history
        .active_leaf(session)?
        .ok_or_else(|| AppError::invalid("Ta rozmowa nie ma jeszcze wiadomości."))?;
    let mut agent_turns = Vec::new();
    let segments: Vec<(Option<u64>, String)> = history
        .branch_projection(session, leaf)?
        .into_iter()
        .filter(|t| !t.hidden && !t.content.text.trim().is_empty())
        .filter_map(|t| {
            let who = match t.role {
                Role::User => "Użytkownik",
                Role::Assistant if with_agents => {
                    agent_turns.push(t.id.0);
                    "Agentka"
                }
                Role::Assistant | Role::System | Role::Tool => return None,
            };
            Some((Some(t.id.0), format!("{who}: {}", t.content.text)))
        })
        .collect();
    Ok(TeachPlan {
        title,
        provenance: Provenance::User,
        source: format!("rozmowa:{session}"),
        chunks: chunk(&segments, limits),
        agent_turns,
    })
}

impl MemoryApp {
    /// `memory_teach`: zapisuje fakty z tekstu, pliku albo rozmowy (jako właściciel).
    pub fn teach(
        &self,
        request: TeachRequest,
        history: &dyn SessionHistory,
    ) -> Result<TeachReport, AppError> {
        self.teach_with(request, history, Limits::default())
    }

    /// Jak [`Self::teach`], z podanymi limitami (testy).
    pub fn teach_with(
        &self,
        request: TeachRequest,
        history: &dyn SessionHistory,
        limits: Limits,
    ) -> Result<TeachReport, AppError> {
        let id = request.session_id.trim();
        if id.is_empty() {
            return Err(AppError::invalid("Brak rozmowy, do której należy nauka."));
        }
        if let TeachSource::Text { text } | TeachSource::File { text, .. } = &request.source
            && text.len() > MAX_REQUEST_BYTES
        {
            return Err(AppError::invalid(
                "Tekst jest za długi do nauki (najwyżej 4 MB) — podziel go na części.",
            ));
        }
        let session = SessionId::new(id);
        let meta = self.p.catalog.session(&session)?;
        let title = request.title.as_deref();
        let conversation = matches!(request.source, TeachSource::Conversation);
        let scope = self.teach_scope(&session, request.scope, conversation)?;
        let plan = match &request.source {
            TeachSource::Conversation => conversation_plan(
                history,
                &session,
                clean_title(title, &format!("Rozmowa „{}”", meta.title)),
                limits,
                matches!(scope, MemoryScope::Session(_)),
            )?,
            external => external_plan(external, title, limits),
        };
        if plan.chunks.pieces.is_empty() {
            return Err(AppError::invalid("Nie ma treści do nauki."));
        }
        let stored = self.store(&plan, &scope, &session)?;
        self.changed(Some(&scope));
        Ok(TeachReport {
            facts: stored.len() as u64,
            ids: stored.iter().map(entry_dto).collect(),
            scope_key: scope_key(&scope),
            source: plan.source.clone(),
            trusted: plan.trusted(),
            truncated: plan.chunks.truncated,
            duplicates: plan.chunks.duplicates as u64,
        })
    }

    fn teach_scope(
        &self,
        session: &SessionId,
        scope: RememberScope,
        trusted: bool,
    ) -> Result<MemoryScope, AppError> {
        if !trusted && scope != RememberScope::Session {
            return Err(AppError::invalid(
                "Tekst i pliki to treść z zewnątrz (niezaufana) — trafiają tylko do pamięci tej rozmowy.",
            ));
        }
        Ok(match scope {
            RememberScope::Session => MemoryScope::Session(session.clone()),
            RememberScope::Global => MemoryScope::Global,
            RememberScope::Project => {
                MemoryScope::Project(self.p.access.project_of(session).ok_or_else(|| {
                    AppError::invalid("Ta sesja nie należy do projektu — wybierz inny zakres.")
                })?)
            }
            RememberScope::Agent => {
                return Err(AppError::invalid(
                    "Naukę zapisuję w pamięci rozmowy, projektu albo globalnej — wybierz jeden z tych zakresów.",
                ));
            }
        })
    }

    /// Zapis faktów; błąd → zapomnienie już zapisanych (wszystko albo nic).
    fn store(
        &self,
        plan: &TeachPlan,
        scope: &MemoryScope,
        session: &SessionId,
    ) -> Result<Vec<EntryRef>, AppError> {
        let mut stored = Vec::new();
        for (i, piece) in plan.chunks.pieces.iter().enumerate() {
            let text = plan.fact_text(i).unwrap_or_default();
            let provenance = plan.provenance_of(i);
            let trusted = provenance.is_trusted();
            let mut new = NewMemory::new(scope.clone(), Layer::Semantic, text, provenance);
            new.entities = vec![plan.title.clone()];
            if !trusted {
                new.confidence = UNTRUSTED_CONFIDENCE;
            }
            if let Some(turn) = piece.turn {
                new.origin = Origin::from_turn(session.clone(), turn);
            }
            match self
                .p
                .service
                .remember_as(&Accessor::Owner, new, RememberMode::Explicit)
            {
                Ok(entry) => stored.push(entry.entry_ref()),
                Err(e) => {
                    for r in stored {
                        if let Err(undo) = self
                            .p
                            .service
                            .forget_as(&Accessor::Owner, &ForgetTarget::Entry(r))
                        {
                            tracing::warn!(error = %undo, "nauka: nie cofnięto zapisanego faktu");
                        }
                    }
                    return Err(e.into());
                }
            }
        }
        Ok(stored)
    }
}
