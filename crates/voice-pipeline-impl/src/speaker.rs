//! Zasób „głośnik” automatu dialogu na `scheduler-lite`: każda wypowiedź agentki trzyma dzierżawę
//! `Resource::Speaker` (posiadaczka = persona, priorytet zwykłej odpowiedzi). Jedna agentka mówi
//! naraz także między sesjami i modułami (narracja, przekazania) — rozstrzyga scheduler.

use std::sync::{Arc, Mutex, MutexGuard};

use personas_contract::PersonaId;
use scheduler_lite_contract::{Holder, Lease, Priority, Resource, SchedulerLite};
use voice_dialog_contract::{SpeakerBusy, SpeakerLock, SpeakerOwner, UtteranceId};
use voice_wake_contract::lease_now;

/// `SpeakerLock` na dzierżawie `scheduler-lite`.
pub struct SchedSpeakerLock {
    scheduler: Arc<dyn SchedulerLite>,
    held: Mutex<Option<(SpeakerOwner, Lease)>>,
}

impl std::fmt::Debug for SchedSpeakerLock {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SchedSpeakerLock")
            .field("holder", &self.holder())
            .finish_non_exhaustive()
    }
}

fn owner_of(holder: &Holder) -> SpeakerOwner {
    let persona = match holder {
        Holder::Persona(p) => p.clone(),
        other => PersonaId::new(other.to_string()),
    };
    SpeakerOwner {
        persona,
        utterance: UtteranceId(0),
    }
}

impl SchedSpeakerLock {
    /// Zasób na schedulerze.
    pub fn new(scheduler: Arc<dyn SchedulerLite>) -> Self {
        Self {
            scheduler,
            held: Mutex::new(None),
        }
    }

    fn lock(&self) -> MutexGuard<'_, Option<(SpeakerOwner, Lease)>> {
        self.held.lock().unwrap_or_else(|p| p.into_inner())
    }

    /// Dzierżawa odebrana (kill-switch) albo scheduler prosi o zwolnienie (wywłaszczenie) —
    /// potok zatrzymuje mowę w najbliższym kroku.
    pub fn lost(&self) -> bool {
        self.lock()
            .as_ref()
            .is_some_and(|(_, l)| l.is_revoked() || l.preempt_requested())
    }
}

impl SpeakerLock for SchedSpeakerLock {
    fn try_acquire(&self, owner: &SpeakerOwner) -> Result<(), SpeakerBusy> {
        let mut held = self.lock();
        if let Some((current, _)) = held.as_ref() {
            return if current == owner {
                Ok(())
            } else {
                Err(SpeakerBusy {
                    holder: current.clone(),
                })
            };
        }
        let lease = lease_now(
            self.scheduler.as_ref(),
            Resource::Speaker,
            Holder::Persona(owner.persona.clone()),
            Priority::Normal,
        );
        match lease {
            Ok(lease) => {
                *held = Some((owner.clone(), lease));
                Ok(())
            }
            Err(_) => Err(SpeakerBusy {
                holder: self
                    .scheduler
                    .holder(&Resource::Speaker)
                    .map_or_else(|| owner.clone(), |info| owner_of(&info.holder)),
            }),
        }
    }

    fn release(&self, owner: &SpeakerOwner) -> bool {
        let mut held = self.lock();
        if held.as_ref().is_some_and(|(o, _)| o == owner) {
            *held = None;
            true
        } else {
            false
        }
    }

    fn holder(&self) -> Option<SpeakerOwner> {
        if let Some((o, _)) = self.lock().as_ref() {
            return Some(o.clone());
        }
        self.scheduler
            .holder(&Resource::Speaker)
            .map(|info| owner_of(&info.holder))
    }
}
