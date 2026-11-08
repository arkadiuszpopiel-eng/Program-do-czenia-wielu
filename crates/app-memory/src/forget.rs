//! Zapomnienie z podglądem kaskady (Inspektor: „zapomnij" pokazuje najpierw, co zniknie —
//! wersje, pochodne w zakresach szerszych — i co wróci do stanu aktywnego). Podgląd liczy ten sam
//! czysty plan co silnik (`plan_cascade`) na wszystkich wpisach widzianych przez właściciela.

use std::collections::BTreeSet;

use app_api::AppError;
use app_api::dto::{MemoryCascadeItem, MemoryForgetPreview, MemoryForgetTarget};
use memory_contract::{
    Accessor, EntryRef, ForgetTarget, InspectorQuery, MAX_PAGE, MemoryEntry, MemoryScope,
    MemoryService, plan_cascade, scope_key,
};

use crate::ids::{entry_dto, parse_entry, parse_scope};

/// Wszystkie wpisy (strony Inspektora po `MAX_PAGE`).
pub fn all_entries(service: &dyn MemoryService) -> Result<Vec<MemoryEntry>, AppError> {
    let mut out = Vec::new();
    loop {
        let query = InspectorQuery {
            offset: out.len(),
            limit: MAX_PAGE,
            ..InspectorQuery::default()
        };
        let page = service.inspect(&Accessor::Owner, &query)?;
        let n = page.items.len();
        out.extend(page.items.into_iter().map(|i| i.entry));
        if n == 0 || out.len() >= page.total {
            break;
        }
    }
    Ok(out)
}

/// Cel DTO → cel kontraktu.
pub fn target_of(target: &MemoryForgetTarget) -> Result<ForgetTarget, AppError> {
    Ok(match target {
        MemoryForgetTarget::Entry { id } => ForgetTarget::Entry(parse_entry(id)?),
        MemoryForgetTarget::Scope { scope } => ForgetTarget::Scope(parse_scope(scope)?),
    })
}

fn short(text: &str) -> String {
    let mut s: String = text.chars().take(120).collect();
    if text.chars().count() > 120 {
        s.push('…');
    }
    s
}

/// Podgląd kaskady.
pub fn preview(
    service: &dyn MemoryService,
    target: &MemoryForgetTarget,
) -> Result<MemoryForgetPreview, AppError> {
    let all = all_entries(service)?;
    let (seeds, family, shred): (BTreeSet<EntryRef>, bool, bool) = match target_of(target)? {
        ForgetTarget::Entry(entry) => {
            if !all.iter().any(|e| e.entry_ref() == entry) {
                return Err(AppError::not_found("Ten wpis pamięci już nie istnieje."));
            }
            (BTreeSet::from([entry]), true, false)
        }
        ForgetTarget::Scope(scope) => {
            let seeds = all
                .iter()
                .filter(|e| e.scope == scope)
                .map(MemoryEntry::entry_ref)
                .collect();
            (seeds, false, !matches!(scope, MemoryScope::Session(_)))
        }
        _ => return Err(AppError::invalid("Nieobsługiwany cel podglądu.")),
    };
    let plan = plan_cascade(&all, &seeds, family);
    let remove = all
        .iter()
        .filter(|e| plan.remove.contains(&e.entry_ref()))
        .map(|e| {
            let r = e.entry_ref();
            let reason = if seeds.contains(&r) {
                "target"
            } else if plan.versions.contains(&r) {
                "version"
            } else {
                "derived"
            };
            MemoryCascadeItem {
                id: entry_dto(&r),
                scope_key: scope_key(&e.scope),
                text: short(&e.text),
                reason: reason.into(),
            }
        })
        .collect();
    Ok(MemoryForgetPreview {
        target: target.clone(),
        remove,
        revive: plan.revive.iter().map(entry_dto).collect(),
        shred,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use memory_contract::{Layer, NewMemory, Origin, Provenance, RememberMode, SessionId};

    #[test]
    fn preview_lists_versions_and_derived_copies() {
        let service = memory_fake::service();
        let a = SessionId::new("a");
        let fact = service
            .remember_as(
                &Accessor::Owner,
                NewMemory::user_fact(a.clone(), "Lubię kawę"),
                RememberMode::Explicit,
            )
            .unwrap();
        let copy = service
            .promote_as(&Accessor::Owner, &fact.entry_ref(), MemoryScope::Global)
            .unwrap();
        let mut other = NewMemory::new(
            MemoryScope::Session(a),
            Layer::Semantic,
            "Inny fakt",
            Provenance::User,
        );
        other.origin = Origin::default();
        service
            .remember_as(&Accessor::Owner, other, RememberMode::Explicit)
            .unwrap();
        let target = MemoryForgetTarget::Entry {
            id: entry_dto(&fact.entry_ref()),
        };
        let p = preview(&service, &target).unwrap();
        let ids: Vec<&str> = p.remove.iter().map(|i| i.id.as_str()).collect();
        assert_eq!(p.remove.len(), 2, "{ids:?}");
        assert!(ids.contains(&entry_dto(&copy.entry_ref()).as_str()));
        assert!(p.remove.iter().any(|i| i.reason == "derived") && !p.shred);
        let report = service
            .forget_as(&Accessor::Owner, &target_of(&target).unwrap())
            .unwrap();
        assert_eq!(report.removed.len(), 2);
        let scope = MemoryForgetTarget::Scope {
            scope: "global".into(),
        };
        assert!(preview(&service, &scope).unwrap().shred);
        assert!(preview(&service, &target).is_err());
    }
}
