//! Zdania „dlaczego to pamiętam” (PL, bez treści innych wpisów).

use crate::inspect::SourceLink;
use crate::model::{Derivation, EntryState, SupersedeReason, expires_at, scope_key};
use crate::types::{Layer, MemoryEntry, Provenance};

fn derivation_name(d: Derivation) -> &'static str {
    match d {
        Derivation::Extracted => "fakt wyekstrahowany przez Strażniczkę pamięci",
        Derivation::Summary => "streszczenie",
        Derivation::Skill => "umiejętność wyuczona z epizodów",
        Derivation::Promoted => "kopia awansowana za zgodą",
        Derivation::Edited => "nowa wersja po edycji",
        Derivation::Imported => "import z paczki .alfa",
    }
}

fn layer_name(l: Layer) -> &'static str {
    match l {
        Layer::Working => "robocza",
        Layer::Episodic => "epizodyczna",
        Layer::Semantic => "semantyczna",
        Layer::Procedural => "proceduralna",
    }
}

/// Powody pamiętania wpisu.
pub fn reasons(
    entry: &MemoryEntry,
    state: EntryState,
    sources: &[SourceLink],
    versions: usize,
) -> Vec<String> {
    let when = entry.created_at.format("%Y-%m-%d %H:%M UTC");
    let who = match &entry.provenance {
        Provenance::User => "Zapisałaś/zapisałeś to jako użytkownik".to_owned(),
        Provenance::Agent { agent } => format!("Zapisała to agentka {agent}"),
        Provenance::Import { source } => format!("Zaimportowano z {source}"),
        Provenance::UntrustedContent { source } => {
            format!("Pochodzi z treści niezaufanej ({source})")
        }
    };
    let mut out = vec![format!(
        "{who} ({when}); warstwa {}, zakres {}.",
        layer_name(entry.layer),
        scope_key(&entry.scope)
    )];
    if let Some(session) = &entry.origin.session {
        match entry.origin.turn {
            Some(turn) => out.push(format!("Źródło: sesja {session}, tura {turn}.")),
            None => out.push(format!("Źródło: sesja {session}.")),
        }
    }
    if let Some(d) = entry.origin.derivation {
        let alive = sources.iter().filter(|s| s.exists).count();
        out.push(format!(
            "To {} z {} wpisów źródłowych ({} nadal istnieje).",
            derivation_name(d),
            sources.len(),
            alive
        ));
    }
    if !entry.trusted {
        out.push("Treść niezaufana — nie awansuje do zakresów szerszych i nie była zapamiętana automatycznie.".into());
    }
    out.push(match state {
        EntryState::Active => "Zatwierdzone i aktywne — może wrócić w odpowiedziach.".into(),
        EntryState::Pending => "Czeka na Twoje zatwierdzenie — agentki go nie widzą.".into(),
        EntryState::Expired => "Wygasło (TTL) — zostanie usunięte przy retencji.".into(),
        EntryState::Superseded => match entry.superseded.as_ref().map(|s| s.reason) {
            Some(SupersedeReason::Duplicate) => "Scalone z innym wpisem (duplikat).".into(),
            Some(SupersedeReason::Edit) => "Zastąpione nowszą wersją po edycji.".into(),
            _ => "Zastąpione nowszą wersją (sprzeczność).".into(),
        },
    });
    if entry.pinned {
        out.push("Przypięte — zawsze w zestawie roboczym rozmowy.".into());
    }
    if versions > 1 {
        out.push(format!("Wersja {} z {versions} w historii.", entry.version));
    }
    if let Some(at) = expires_at(entry) {
        out.push(format!("Wygasa {}.", at.format("%Y-%m-%d")));
    }
    out.push(format!("Pewność: {:.0}%.", entry.confidence * 100.0));
    out
}
