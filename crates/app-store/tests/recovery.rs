//! Trwały punkt wznowienia w szyfrowanej bazie sesji.
#![allow(clippy::unwrap_used, clippy::expect_used)]
use app_store::AppStore;
use lib_sqlstore::{Db, DbKey};
use sessions_contract::{SessionDbProvider, SessionError, SessionId};
use std::sync::Arc;
struct Provider(Arc<Db>);
impl SessionDbProvider for Provider {
    fn session_db(&self, _: &SessionId) -> Result<Arc<Db>, SessionError> {
        Ok(self.0.clone())
    }
    fn session_ids(&self) -> Result<Vec<SessionId>, SessionError> {
        Ok(vec![SessionId::new("s")])
    }
}
#[test]
fn recovery_state_survives_close_and_reopen_in_encrypted_database() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("session.db");
    let key = DbKey::from_bytes([7; 32]);
    let sid = SessionId::new("s");
    {
        let store = AppStore::new(Arc::new(Provider(Arc::new(Db::open(&path, &key).unwrap()))));
        store
            .save_work(
                &sid,
                "r1",
                "runtime",
                Some("turn1"),
                &serde_json::json!({"seq":2,"goal":"private-goal","done":["write1"]}),
            )
            .unwrap();
    }
    let bytes = std::fs::read(&path).unwrap();
    assert!(!bytes.windows(12).any(|b| b == b"private-goal"));
    let store = AppStore::new(Arc::new(Provider(Arc::new(Db::open(&path, &key).unwrap()))));
    let work = store.work(&sid, "r1").unwrap().unwrap();
    assert_eq!(work["seq"], 2);
    assert_eq!(store.work_for_turn(&sid, "turn1").unwrap().len(), 1);
    store
        .save_work(
            &sid,
            "r1",
            "runtime",
            Some("turn1"),
            &serde_json::json!({"seq":3}),
        )
        .unwrap();
    assert_eq!(store.work(&sid, "r1").unwrap().unwrap()["seq"], 3);
}
