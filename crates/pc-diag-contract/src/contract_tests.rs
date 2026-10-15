//! Test kontraktowy sondy parametrów (uruchamiany na `-fake` i prawdziwej sondzie).

use crate::{DiagProbe, Snapshot};

/// Zapis spójny: procesor i pamięć opisane, woluminy w granicach rozmiaru, brak adresów sieci.
pub fn snapshot_is_consistent<P: DiagProbe + ?Sized>(probe: &P) -> Snapshot {
    let s = probe.snapshot().expect("zapis parametrów");
    assert!(!s.cpu.name.is_empty(), "nazwa procesora");
    assert!(
        s.cpu.threads >= s.cpu.cores && s.cpu.cores > 0,
        "rdzenie i wątki"
    );
    assert!(s.memory.total_mb > 0 && s.memory.available_mb <= s.memory.total_mb);
    for v in &s.volumes {
        assert!(v.free_mb <= v.size_mb, "wolne ≤ rozmiar ({})", v.letter);
        assert!(
            v.letter.len() == 2 && v.letter.ends_with(':'),
            "litera {}",
            v.letter
        );
    }
    let json = serde_json::to_string(&s).expect("JSON");
    for forbidden in ["IPAddress", "MACAddress", "\"mac\"", "\"ip\""] {
        assert!(
            !json.contains(forbidden),
            "bez adresów sieciowych: {forbidden}"
        );
    }
    assert!(s.events.recent.len() <= 30);
    s
}
