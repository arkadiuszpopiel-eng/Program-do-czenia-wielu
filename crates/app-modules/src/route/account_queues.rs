//! Mapowanie kolejek kont na trasy.
use super::global_queue::{SharedQueues, class_of};
use accounts_hub_contract::Account;
use router_contract::{Candidate, TaskClass};
use router_impl::GlobalQueues;
use std::collections::BTreeMap;
/// Klasa zadań huba → klasa Routera (te same nazwy; osobne typy dwóch kontraktów).
fn route_class(class: accounts_hub_contract::TaskClass) -> TaskClass {
    match class {
        accounts_hub_contract::TaskClass::VoiceFast => TaskClass::VoiceFast,
        accounts_hub_contract::TaskClass::Conversation => TaskClass::Conversation,
        accounts_hub_contract::TaskClass::Code => TaskClass::Code,
        accounts_hub_contract::TaskClass::Planning => TaskClass::Planning,
        accounts_hub_contract::TaskClass::GuiVision => TaskClass::GuiVision,
        accounts_hub_contract::TaskClass::Summarize => TaskClass::Summarize,
        accounts_hub_contract::TaskClass::Embeddings => TaskClass::Embeddings,
    }
}

/// Wspólna kolejka → kandydaci aktywnych kont, w kolejności właściciela.
pub(super) fn global_candidates(
    shared: &SharedQueues,
    chosen: &BTreeMap<String, Account>,
) -> GlobalQueues {
    shared
        .iter()
        .filter_map(|(name, entries)| {
            let class = class_of(name)?;
            let list: Vec<Candidate> = entries
                .iter()
                .filter_map(|entry| {
                    let (provider, _) = chosen
                        .iter()
                        .find(|(_, account)| account.id.as_str() == entry.account_id)?;
                    Some(Candidate::new(provider.as_str(), entry.model.clone()))
                })
                .collect();
            (!list.is_empty()).then_some((class, list))
        })
        .collect()
}

pub(super) fn queues_of(account: &Account) -> BTreeMap<TaskClass, Vec<String>> {
    account
        .queues
        .iter()
        .filter(|(_, models)| !models.is_empty())
        .map(|(class, models)| {
            (
                route_class(*class),
                models.iter().map(|m| m.as_str().to_owned()).collect(),
            )
        })
        .collect()
}
