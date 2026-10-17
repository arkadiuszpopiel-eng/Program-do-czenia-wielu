---
name: awaria-2026-10-17-obciazenie
description: Awaria PC 2026-10-17 20:31 przy pełnych testach cargo — brak pamięci + resety USB dysku E:; jak teraz uruchamiać ciężkie zadania
metadata:
  node_type: memory
  type: project
  originSessionId: 8c1892ed-2987-4f4f-91ab-0abe9bb3c4b8
  modified: 2026-10-17T21:41:57.210Z
---

2026-10-17 20:31 komputer zawiesił się i zrestartował (Kernel-Power 41, bez BSOD) w trakcie pełnego
`cargo test` 14 crate'ów. Przed awarią: brak pamięci wirtualnej (rustc ×2 po 2,4 GB + `vmmem` 4,3 GB),
1452 ponowienia we/wy i 104 resety mostka USB (UASPStor 129) na dysku E: (ADATA na USB), a dysk F:
(Realtek RTL9210B) zgłosił nieudany opóźniony zapis `$Mft`. Wcześniej tego dnia E: zapełnił się do 0 GB
(target/ ~111 GB). Po restarcie: E: i F: czyste (`Repair-Volume -Scan`), `git fsck --full` czysty,
wszystkie niezacommitowane pliki całe.

**Why:** ten PC (32 GB RAM, dyski projektu na USB) nie wytrzymuje domyślnej równoległości cargo przy
pełnych testach.
**How to apply:** ciężkie biegi z `CARGO_BUILD_JOBS=6` i `CARGO_INCREMENTAL=0`; nie łączyć cargo z testami
w przeglądarce; pilnować miejsca na E: (stare `target/debug/deps` sprzed doby usuwać regułą z status.md;
ten sam wieczór `target\debug` znów urósł do 170 GB — każda zmiana ustawień cargo tworzy nowy komplet
plików testowych; bezpiecznie usuwać starsze duplikaty `*.exe`+`*.pdb`+`*.d` tej samej nazwy w `deps`
oraz `target/debug/incremental`, gdy nie działa cargo; `verify` pomija etapy Rust przy małym miejscu);
po awarii najpierw sprawdzić dzienniki, dyski, git i pliki. Narzędzia Rust tylko po
`. .\scripts\portable-env.ps1` — bez niego rustup w katalogu repo instaluje 1.94 do `~/.rustup`
(zostawia niedokończoną instalację). Skrypt `scripts/backup.sh` naprawiony: `git -c safe.directory=*`
(inaczej paczka git po cichu się nie tworzyła). Zob. [[maszyna-stacjonarna-2026-10-17]].
