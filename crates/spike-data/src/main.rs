//! Pomiar spike'u (i): rozmiar pliku i czasy 100 wstawień / zapytań kNN i FTS5 (wynik na stdout).

#![allow(clippy::print_stdout)]

use std::time::Instant;

use spike_data::{
    embedding_for, fts_search, init_schema, insert_messages, knn, open, remove_database,
    sample_messages, versions, SpikeError,
};

const N: usize = 100;
const QUERIES: usize = 100;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let dir = std::env::temp_dir().join(format!("spike-data-{}", std::process::id()));
    std::fs::create_dir_all(&dir)?;
    let path = dir.join("sesja.db");
    let key = "klucz-sesji-do-pomiaru";

    let mut conn = open(&path, Some(key))?;
    let (sqlite, cipher, vec) = versions(&conn)?;
    println!("SQLite {sqlite} · SQLCipher {cipher} · sqlite-vec {vec}");
    init_schema(&conn)?;

    let messages = sample_messages("s1", N);
    let t = Instant::now();
    insert_messages(&mut conn, &messages)?;
    println!(
        "wstawienia: {N} rekordów (3 tabele, 1 transakcja) w {:?}",
        t.elapsed()
    );

    let t = Instant::now();
    let mut hits = 0;
    for i in 1..=QUERIES {
        let res = knn(&conn, &embedding_for(i as i64), 5)?;
        if res.first().map(|(id, _)| *id) == Some(i as i64) {
            hits += 1;
        }
    }
    println!(
        "kNN: {QUERIES} zapytań (k=5) w {:?}, trafień top-1: {hits}/{QUERIES}",
        t.elapsed()
    );

    let t = Instant::now();
    let mut total = 0;
    for _ in 0..QUERIES {
        total += fts_search(&conn, "żółć")?.len();
    }
    println!(
        "FTS5: {QUERIES} zapytań „żółć” w {:?}, trafień łącznie: {total}",
        t.elapsed()
    );
    for q in ["żółć", "zolc", "zółc", "sesja", "gęś", "ges", "jaźń"] {
        println!("  FTS5 „{q}” → {} trafień", fts_search(&conn, q)?.len());
    }

    drop(conn);
    let size = std::fs::metadata(&path)?.len();
    println!(
        "rozmiar pliku po zamknięciu (WAL scalony): {size} B ({:.1} KiB)",
        size as f64 / 1024.0
    );

    let t = Instant::now();
    let no_key = open(&path, None).map(|_| ()).map_err(|e| e.to_string());
    println!("otwarcie bez klucza: {no_key:?} ({:?})", t.elapsed());
    let bad = open(&path, Some("zly-klucz"))
        .map(|_| ())
        .map_err(|e| e.to_string());
    println!("otwarcie ze złym kluczem: {bad:?}");
    let t = Instant::now();
    open(&path, Some(key)).map(|_| ())?;
    println!("ponowne otwarcie z dobrym kluczem: OK ({:?})", t.elapsed());

    remove_database(&path)?;

    // Klucz surowy (64 hex = 32 B) omija KDF (PBKDF2-HMAC-SHA512, 256 000 iteracji) — klucz per sesja
    // i tak leży w Credential Manager, więc może być losowym kluczem binarnym.
    let raw_path = dir.join("sesja-raw.db");
    let raw_key = format!("x'{}'", "ab".repeat(32));
    open(&raw_path, Some(&raw_key)).map(|_| ())?;
    let t = Instant::now();
    open(&raw_path, Some(&raw_key)).map(|_| ())?;
    println!(
        "otwarcie z kluczem surowym (bez KDF): OK ({:?})",
        t.elapsed()
    );
    remove_database(&raw_path)?;
    std::fs::remove_dir_all(&dir)?;
    Ok::<(), SpikeError>(()).map_err(Into::into)
}
