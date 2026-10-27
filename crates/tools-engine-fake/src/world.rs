//! Świat atrapy: encje, rewizja, klatka, historia cofania, dziennik transakcji, zaznaczenie.
//! Reguły jak w serwerze (`engine.rs` silnika): transakcja atomowa na kopii, ten sam
//! identyfikator z tą samą treścią zwraca zapisane pokwitowanie, z inną — `duplicate_transaction`.

use std::collections::BTreeMap;

use serde_json::{Value, json};
use tools_engine_contract::{EngineError, EngineMethod, Scene};

/// Największa historia cofania (jak w serwerze).
const HISTORY_MAX: usize = 64;

fn rpc(kind: &str, message: impl Into<String>) -> EngineError {
    EngineError::Rpc {
        code: if kind == "invalid_params" {
            -32602
        } else {
            -32000
        },
        kind: kind.into(),
        message: message.into(),
    }
}

/// Świat w pamięci.
pub struct World {
    entities: BTreeMap<u64, Value>,
    next_id: u64,
    revision: u64,
    frame: u64,
    history: Vec<(BTreeMap<u64, Value>, u64)>,
    journal: BTreeMap<String, (Value, Value)>,
    selection: Option<u64>,
}

impl World {
    /// Świat sceny (pokazowa ma jedną encję podłoża).
    pub fn new(scene: Scene) -> Self {
        let mut w = Self {
            entities: BTreeMap::new(),
            next_id: 1,
            revision: 0,
            frame: 0,
            history: Vec::new(),
            journal: BTreeMap::new(),
            selection: None,
        };
        if scene != Scene::Empty {
            w.insert(json!({"tag": "podłoże", "dimension": "3d"}));
        }
        w
    }

    /// Bieżąca rewizja.
    pub fn revision(&self) -> u64 {
        self.revision
    }

    fn insert(&mut self, spec: Value) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        let mut e = spec;
        e["id"] = json!(id);
        self.entities.insert(id, e);
        id
    }

    fn target(op: &Value) -> Result<u64, EngineError> {
        op["target_id"]
            .as_u64()
            .ok_or_else(|| rpc("invalid_params", "target_id: liczba"))
    }

    fn apply(&mut self, op: &Value, spawned: &mut Vec<u64>) -> Result<(), EngineError> {
        match op["op"].as_str().unwrap_or_default() {
            "entity.spawn" => {
                let spec = op.get("entity").cloned().unwrap_or_else(|| json!({}));
                spawned.push(self.insert(spec));
            }
            "entity.despawn" => {
                let id = Self::target(op)?;
                self.entities
                    .remove(&id)
                    .ok_or_else(|| rpc("entity_not_found", format!("encja {id}")))?;
            }
            "component.set_transform" | "component.set_property" | "render.set_material" => {
                let id = Self::target(op)?;
                let e = self
                    .entities
                    .get_mut(&id)
                    .ok_or_else(|| rpc("entity_not_found", format!("encja {id}")))?;
                if let Some(t) = op.get("transform") {
                    e["transform"] = t.clone();
                }
            }
            "" => return Err(rpc("invalid_params", "operacja bez pola op")),
            // Pozostałe operacje silnika atrapa przyjmuje bez skutków w świecie.
            _ => {}
        }
        Ok(())
    }

    fn transaction(&mut self, p: &Value) -> Result<Value, EngineError> {
        let id = p["transaction_id"]
            .as_str()
            .filter(|s| !s.is_empty() && s.len() <= 128)
            .ok_or_else(|| rpc("invalid_input", "transaction_id: 1–128 znaków"))?
            .to_owned();
        let body = json!([p["expected_revision"], p["operations"]]);
        if let Some((seen, receipt)) = self.journal.get(&id) {
            return if *seen == body {
                Ok(receipt.clone())
            } else {
                Err(rpc("duplicate_transaction", format!("{id} z inną treścią")))
            };
        }
        if let Some(expected) = p["expected_revision"].as_u64()
            && expected != self.revision
        {
            return Err(rpc(
                "revision_conflict",
                format!("oczekiwano {expected}, jest {}", self.revision),
            ));
        }
        let ops = p["operations"]
            .as_array()
            .filter(|o| !o.is_empty() && o.len() <= 1024)
            .ok_or_else(|| rpc("invalid_input", "operations: 1–1024"))?;
        let before = (self.entities.clone(), self.next_id);
        let mut spawned = Vec::new();
        for op in ops {
            if let Err(e) = self.apply(op, &mut spawned) {
                (self.entities, self.next_id) = before;
                return Err(e);
            }
        }
        self.history.push(before);
        if self.history.len() > HISTORY_MAX {
            self.history.remove(0);
        }
        self.revision += 1;
        let receipt = json!({"transaction_id": id, "revision": self.revision, "frame": self.frame,
                             "spawned_entities": spawned, "snapshot_saved": true});
        self.journal.insert(id, (body, receipt.clone()));
        Ok(receipt)
    }

    fn streaming(&self) -> Value {
        json!({"enabled": false, "ready": true, "near_ready": true, "resident_chunks": 0,
               "revision": self.revision})
    }

    /// Wywołanie metody serwera.
    pub fn call(&mut self, method: EngineMethod, p: Value) -> Result<Value, EngineError> {
        match method {
            EngineMethod::ExecuteTransaction => self.transaction(&p),
            EngineMethod::HotReloadShader => {
                let op = json!({"op": "shader.set", "shader_name": p["shader_name"],
                                "wgsl_code": p["wgsl_code"]});
                let tx = json!({"transaction_id": p["transaction_id"],
                                "expected_revision": p["expected_revision"], "operations": [op]});
                let receipt = self.transaction(&tx)?;
                Ok(
                    json!({"receipt": receipt, "validated": true, "gpu_applied": false,
                          "status": "validated_headless"}),
                )
            }
            EngineMethod::RollbackLastChange => {
                let (entities, next_id) = self
                    .history
                    .pop()
                    .ok_or_else(|| rpc("no_snapshot", "historia pusta"))?;
                (self.entities, self.next_id) = (entities, next_id);
                self.revision += 1;
                Ok(json!({"revision": self.revision, "frame": self.frame,
                          "history_len": self.history.len()}))
            }
            EngineMethod::HeadlessStep => {
                let frames = p["frames"]
                    .as_u64()
                    .filter(|f| *f <= 10_000)
                    .ok_or_else(|| rpc("invalid_params", "frames: 0–10000"))?;
                self.frame += frames;
                if frames > 0 {
                    self.revision += 1;
                }
                Ok(
                    json!({"frame": self.frame, "revision": self.revision, "steps": frames,
                          "fixed_dt_seconds": 1.0 / 60.0}),
                )
            }
            EngineMethod::SetSpatialSelection => {
                let id = p["entity_id"].as_u64();
                if let Some(id) = id.filter(|i| !self.entities.contains_key(i)) {
                    return Err(rpc("entity_not_found", format!("encja {id}")));
                }
                self.selection = id;
                self.revision += 1;
                Ok(json!({"revision": self.revision}))
            }
            EngineMethod::GetSpatialSelection => Ok(json!({
                "selection": self.selection.and_then(|i| self.entities.get(&i)).cloned(),
                "revision": self.revision, "source": "atrapa"})),
            EngineMethod::InspectSceneTelemetry => {
                let limit = p["limit"].as_u64().unwrap_or(64) as usize;
                let all: Vec<_> = self.entities.values().cloned().collect();
                Ok(
                    json!({"mode": "headless", "revision": self.revision, "frame": self.frame,
                          "entity_count": all.len(), "truncated": all.len() > limit,
                          "entities": all.into_iter().take(limit).collect::<Vec<_>>(),
                          "history_len": self.history.len()}),
                )
            }
            EngineMethod::InspectWorldStreaming | EngineMethod::StreamWorld => Ok(self.streaming()),
            EngineMethod::RaycastWorld => Ok(json!({"hit": null, "revision": self.revision,
                                                    "source": "atrapa"})),
            EngineMethod::QueryWorldHeight => {
                Err(rpc("invalid_input", "strumieniowanie terenu wyłączone"))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn failed_transaction_changes_nothing() {
        let mut w = World::new(Scene::Demo);
        let bad = json!({"transaction_id": "t", "operations": [
            {"op": "entity.spawn", "entity": {"tag": "a"}},
            {"op": "entity.despawn", "target_id": 99}]});
        assert!(matches!(w.call(EngineMethod::ExecuteTransaction, bad),
                         Err(EngineError::Rpc { kind, .. }) if kind == "entity_not_found"));
        assert_eq!(w.revision(), 0);
        let t = w
            .call(EngineMethod::InspectSceneTelemetry, json!({}))
            .unwrap();
        assert_eq!(t["entity_count"], 1);
        let dup = json!({"transaction_id": "x", "operations": [{"op": "view.configure"}]});
        w.call(EngineMethod::ExecuteTransaction, dup).unwrap();
        let other = json!({"transaction_id": "x", "operations": [{"op": "dust.configure"}]});
        assert!(matches!(w.call(EngineMethod::ExecuteTransaction, other),
                         Err(EngineError::Rpc { kind, .. }) if kind == "duplicate_transaction"));
        w.call(EngineMethod::RollbackLastChange, json!({})).unwrap();
        assert!(w.call(EngineMethod::RollbackLastChange, json!({})).is_err());
        assert!(
            w.call(EngineMethod::SetSpatialSelection, json!({"entity_id": 7}))
                .is_err()
        );
        w.call(EngineMethod::SetSpatialSelection, json!({"entity_id": 1}))
            .unwrap();
        let sel = w
            .call(EngineMethod::GetSpatialSelection, json!({}))
            .unwrap();
        assert_eq!(sel["selection"]["tag"], "podłoże");
    }
}
