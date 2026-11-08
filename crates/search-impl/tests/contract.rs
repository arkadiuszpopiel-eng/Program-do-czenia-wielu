//! Kontrakt współdzielony (`ACC-F1-search-01..03`) na prawdziwych bazach SQLCipher.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use search_contract::contract_tests;

#[test]
fn contract_suite() {
    contract_tests::run_all(common::harness);
}

#[test]
fn tx_search_suite_on_encrypted_db() {
    use sessions_contract::SessionDbProvider;
    let h = common::harness();
    let db = h
        .provider
        .session_db(&search_contract::SessionId::new("zakres-globalny"))
        .unwrap();
    db.with(|conn| {
        contract_tests::tx_search_suite(&h.search, &h.search, conn);
        Ok::<(), ()>(())
    })
    .unwrap();
}

#[test]
fn compact_purges_deleted_terms_from_fts_internals() {
    use lib_sqlstore::rusqlite::Connection;
    use search_contract::{DocId, DocKind, SessionId, TxIndexer};
    use sessions_contract::SessionDbProvider;
    fn blobs_contain(conn: &Connection, needle: &[u8]) -> bool {
        let tables: Vec<String> = conn
            .prepare("SELECT name FROM sqlite_master WHERE type = 'table'")
            .unwrap()
            .query_map([], |r| r.get(0))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        tables.iter().any(|t| {
            let mut stmt = conn.prepare(&format!("SELECT * FROM \"{t}\"")).unwrap();
            let cols = stmt.column_count();
            let mut rows = stmt.query([]).unwrap();
            let mut found = false;
            while let Some(row) = rows.next().unwrap() {
                for i in 0..cols {
                    let bytes: Vec<u8> = match row.get_ref(i).unwrap() {
                        lib_sqlstore::rusqlite::types::ValueRef::Text(b)
                        | lib_sqlstore::rusqlite::types::ValueRef::Blob(b) => b.to_vec(),
                        _ => Vec::new(),
                    };
                    found |= bytes.windows(needle.len()).any(|w| w == needle);
                }
            }
            found
        })
    }
    let h = common::harness();
    let s = SessionId::new("A");
    let db = h.provider.session_db(&s).unwrap();
    db.with(|conn| {
        h.search.prepare(conn).unwrap();
        for i in 0..5 {
            let d = search_contract::contract_tests::doc(
                "A",
                DocKind::Memory,
                &format!("k{i}"),
                &format!("zwykly tekst {i}"),
            );
            h.search.index_in(conn, &d).unwrap();
        }
        let secret = search_contract::contract_tests::doc(
            "A",
            DocKind::Memory,
            "tajny",
            "sekretnezaklecie qxv",
        );
        h.search.index_in(conn, &secret).unwrap();
        assert!(blobs_contain(conn, b"sekretnezaklecie"));
        h.search
            .remove_in(conn, &s, &DocId::new(DocKind::Memory, "tajny"))
            .unwrap();
        h.search.compact_in(conn).unwrap();
        assert!(
            !blobs_contain(conn, b"sekretnezaklecie"),
            "słowo usuniętego dokumentu w indeksie"
        );
        assert!(blobs_contain(conn, b"zwykly"));
        Ok::<(), ()>(())
    })
    .unwrap();
}
