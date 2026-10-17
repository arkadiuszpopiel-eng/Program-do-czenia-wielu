//! Testy `gpu_mem`: wyjście `nvidia-smi` i odczyt liczników systemu (każdy producent).

use super::*;

#[test]
fn picks_the_card_with_most_free_memory() {
    assert_eq!(parse_free_mb("5368\n"), Some(5368));
    assert_eq!(parse_free_mb("  812 \r\n 11240\r\n"), Some(11240));
    assert_eq!(parse_free_mb(""), None);
    assert_eq!(parse_free_mb("[N/A]\n"), None);
    assert_eq!(parse_free_mb("brak karty\n4096\n"), Some(4096));
}

#[test]
fn cards_are_read_with_missing_readings_and_commas_in_names() {
    let out = "0, NVIDIA GeForce RTX 4050 Laptop GPU, 6141, 549, 5372, 0, 48, 2.33\r\n\
               1, Karta, z przecinkiem, 16384, 1024, 15360, [N/A], [N/A], [N/A]\n\
               zepsuta linia\n2, Bez pamięci, [N/A], 1, 2, 3, 4, 5\n";
    let cards = parse_cards(out);
    assert_eq!(cards.len(), 2);
    assert_eq!(cards[0].name, "NVIDIA GeForce RTX 4050 Laptop GPU");
    assert_eq!(
        (cards[0].total_mb, cards[0].used_mb, cards[0].free_mb),
        (6141, 549, 5372)
    );
    assert_eq!(
        (cards[0].load_pct, cards[0].temp_c, cards[0].power_w),
        (Some(0), Some(48), Some(2))
    );
    assert_eq!(cards[1].name, "Karta, z przecinkiem");
    assert_eq!(
        (cards[1].load_pct, cards[1].temp_c, cards[1].power_w),
        (None, None, None)
    );
}

#[test]
fn processes_are_named_marked_and_ordered() {
    let out = "17000, C:\\Program Files\\NVIDIA Corporation\\NVIDIA app\\CEF\\NVIDIA Overlay.exe, [N/A]\n\
               4242, C:\\Users\\x\\AppData\\Local\\Alfa\\sidecars\\llama-cuda\\llama-server.exe, 2610\n\
               12696, C:\\Windows\\explorer.exe, 120\n\
               4242, C:\\Users\\x\\AppData\\Local\\Alfa\\sidecars\\llama-cuda\\llama-server.exe, 2610\n\
               śmieci\n9, , 5\n";
    let p = parse_processes(out);
    assert_eq!(p.len(), 3);
    assert_eq!(
        (p[0].name.as_str(), p[0].alfa, p[0].used_mb),
        ("llama-server.exe", true, Some(2610))
    );
    assert_eq!(
        (p[1].name.as_str(), p[1].alfa, p[1].used_mb),
        ("explorer.exe", false, Some(120))
    );
    assert_eq!(
        (p[2].name.as_str(), p[2].used_mb),
        ("NVIDIA Overlay.exe", None)
    );
}

fn adapter(
    index: u32,
    name: &str,
    vendor_id: u32,
    vram: u64,
    integrated: Option<bool>,
) -> GpuAdapter {
    GpuAdapter {
        name: name.into(),
        vendor_id,
        device_id: 1,
        dedicated_vram_mb: vram,
        shared_memory_mb: 16_000,
        software: false,
        index,
        luid: u64::from(index) + 100,
        integrated,
        driver_version: None,
    }
}

fn usage_of(luid: u64, dedicated: u64, shared: u64) -> platform_contract::GpuAdapterUsage {
    platform_contract::GpuAdapterUsage {
        luid,
        dedicated_used_mb: dedicated,
        shared_used_mb: shared,
        load_pct: Some(7),
    }
}

/// AMD i Intel bez `nvidia-smi`: wolna pamięć osobnej karty z liczników systemu; karta
/// zintegrowana (także APU z 4 GB z BIOS-u) nie jest liczona jako pamięć karty.
#[test]
fn system_counters_measure_every_vendor() {
    let adapters = vec![
        adapter(0, "AMD Radeon 780M Graphics", 0x1002, 4_096, Some(true)),
        adapter(1, "AMD Radeon RX 9070 XT", 0x1002, 16_304, Some(false)),
        adapter(2, "Microsoft Basic Render Driver", 0x1414, 0, None),
    ];
    let usage = GpuUsage {
        adapters: vec![usage_of(100, 512, 900), usage_of(101, 1_189, 104)],
        processes: vec![
            platform_contract::GpuProcessUsage {
                pid: 7,
                luid: 101,
                dedicated_used_mb: 2_610,
                name: Some("llama-server.exe".into()),
            },
            platform_contract::GpuProcessUsage {
                pid: 7,
                luid: 100,
                dedicated_used_mb: 10,
                name: Some("llama-server.exe".into()),
            },
            platform_contract::GpuProcessUsage {
                pid: 9,
                luid: 101,
                dedicated_used_mb: 300,
                name: None,
            },
        ],
    };
    assert_eq!(free_from(&adapters, &usage), Some(16_304 - 1_189));
    // Bez osobnej karty — brak pomiaru (pamięć zintegrowanej to RAM).
    assert_eq!(free_from(&adapters[..1], &usage), None);
    // Stary system bez flagi: ≥ 1 GB własnej pamięci = osobna karta.
    let old = vec![adapter(1, "Intel Arc A770", 0x8086, 16_224, None)];
    assert_eq!(free_from(&old, &usage), Some(16_224 - 1_189));

    let (cards, processes) = cards_from(&adapters, &usage);
    assert_eq!(cards.len(), 2, "bez adaptera programowego");
    assert_eq!(
        (cards[0].total_mb, cards[0].used_mb, cards[0].load_pct),
        (4_096 + 16_000, 512 + 900, Some(7)),
        "zintegrowana: pamięć współdzielona"
    );
    assert_eq!(
        (cards[1].total_mb, cards[1].free_mb),
        (16_304, 16_304 - 1_189)
    );
    assert_eq!((cards[1].temp_c, cards[1].power_w), (None, None));
    assert_eq!(processes.len(), 2);
    assert_eq!(
        (
            processes[0].name.as_str(),
            processes[0].alfa,
            processes[0].used_mb
        ),
        ("llama-server.exe", true, Some(2_620))
    );
    assert_eq!(processes[1].name, "PID 9");
}
