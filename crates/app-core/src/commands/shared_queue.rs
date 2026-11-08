//! Wspólna kolejka modeli wszystkich kont API (zgłoszenie właściciela 2026-10-14): obok kolejek
//! poszczególnych kont jedna kolejność prób dla klasy zadań, z modelami z różnych kont. Zapis
//! w `model-queues.json` katalogu konfiguracji; Router (`app-modules::route`) czyta go przy każdej
//! turze, więc zmiana działa od następnej odpowiedzi.

use app_modules::route::global_queue::{self, MAX_ENTRIES, QueueEntry};

use crate::core::AppCore;
use crate::dto::{SharedQueueEntry, SharedQueues};
use crate::error::AppError;

fn to_dto(queues: global_queue::SharedQueues) -> SharedQueues {
    queues
        .into_iter()
        .map(|(class, entries)| {
            let entries = entries
                .into_iter()
                .map(|e| SharedQueueEntry {
                    account_id: e.account_id,
                    model: e.model,
                })
                .collect();
            (class, entries)
        })
        .collect()
}

impl AppCore {
    fn shared_queue_file(&self) -> std::path::PathBuf {
        self.inner.paths.config.join(global_queue::FILE)
    }

    /// `accounts_shared_queues`: wspólna kolejka wszystkich klas (pusta klasa — brak wpisu).
    pub async fn accounts_shared_queues(&self) -> Result<SharedQueues, AppError> {
        Ok(to_dto(global_queue::load(&self.shared_queue_file())))
    }

    /// `accounts_set_shared_queue`: wspólna kolejka klasy zadań (najwyżej 10 pozycji, każda to
    /// istniejące konto i nazwa modelu). Pusta lista usuwa kolejkę klasy — wracają kolejki kont.
    pub async fn accounts_set_shared_queue(
        &self,
        task_class: String,
        entries: Vec<SharedQueueEntry>,
    ) -> Result<SharedQueues, AppError> {
        super::accounts::task_from_name(&task_class)?;
        if entries.len() > MAX_ENTRIES {
            return Err(AppError::invalid(format!(
                "Wspólna kolejka może mieć najwyżej {MAX_ENTRIES} modeli."
            )));
        }
        let mut clean: Vec<QueueEntry> = Vec::with_capacity(entries.len());
        for entry in entries {
            let id = super::accounts::account_id(&entry.account_id)?;
            self.account(&id)?;
            let model = entry.model.trim();
            accounts_hub_contract::ModelId::new(model).map_err(|_| {
                AppError::invalid("Nazwa modelu w kolejce jest pusta albo niepoprawna.")
            })?;
            let item = QueueEntry {
                account_id: id.as_str().to_owned(),
                model: model.to_owned(),
            };
            if clean.contains(&item) {
                return Err(AppError::invalid(format!(
                    "Model „{model}” tego konta jest w kolejce dwa razy."
                )));
            }
            clean.push(item);
        }
        let path = self.shared_queue_file();
        let mut queues = global_queue::load(&path);
        if clean.is_empty() {
            queues.remove(&task_class);
        } else {
            queues.insert(task_class, clean);
        }
        global_queue::save(&path, &queues).map_err(|e| {
            AppError::internal(format!("Nie udało się zapisać wspólnej kolejki: {e}"))
        })?;
        Ok(to_dto(queues))
    }
}
