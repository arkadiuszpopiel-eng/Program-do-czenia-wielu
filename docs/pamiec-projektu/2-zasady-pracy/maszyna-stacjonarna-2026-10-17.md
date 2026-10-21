---
name: maszyna-stacjonarna-2026-10-17
description: "Od 2026-10-17 projekt Alfa działa na stacjonarnym PC (Ryzen 5700X3D, RX 9070 XT), nie na laptopie — skutki dla git, node_modules, głosu"
metadata:
  node_type: memory
  type: project
  originSessionId: 8c1892ed-2987-4f4f-91ab-0abe9bb3c4b8
  modified: 2026-10-17T22:02:25.660Z
---

Od 2026-10-17 dysk E: (ADATA SX8200 na USB) z projektem jest podpięty do stacjonarnego PC DESKTOP-S02O9OH
(AMD Ryzen 7 5700X3D, AMD Radeon RX 9070 XT, 32 GB RAM, konto Spopielony). Wcześniej: laptop i7-13620H, RTX 4050.

Skutki stwierdzone przy analizie 2026-10-17:
- Pliki należą do SID ze starej instalacji → git wymaga `-c safe.directory=*` (globalnej konfiguracji nie zmieniano).
- 1447 plików w starym `node_modules` miało EPERM; naprawione 2026-10-17 (`pnpm install` z lockfile do nowego katalogu). Resztki w `Alfa AI Program/stare-node_modules-acl-2026-10-17` (1447 plików starego SID) usunie tylko właściciel z prawami administratora.
- `cargo check --workspace --offline --all-targets` z `portable-env.ps1` (Rust 1.94.1) zielony w 2 min 15 s.
- Brak `%LOCALAPPDATA%\Alfa` (modele, silniki, profil), dysk F: (Kopie Plików, Realtek RTL9210B) podłączany okresowo, brak `gh`, brak Python313 z launch.json.
- GPU AMD: silniki pobrane na laptopie były w wariancie CUDA (whisper) — tu nie zadziałają bez innego wariantu.

2026-10-17 polecenie właściciela: Alfa ma działać na NVIDIA, AMD, Intel i iGPU — zadanie C6.6 w roadmapie
(commit 6ff0c311 + seria 0.0.63: Qwen3-ASR przez Vulkan na AMD/Intel, DirectML dla Supertonic, ADR 0028 proponowany).
node_modules zainstalowane z lockfile (właściciel zgodził się na pobranie brakujących pakietów).
`cargo fmt --all` pada na Windows (os error 206) — używać `rustfmt --edition 2024 <pliki>`.

Pamięć asystenta na tym PC była pusta — zasady pracy z laptopa (alfa-*.md) przywrócone 2026-10-18 z kopii
w repozytorium (`alfa/docs/pamiec-projektu/2-zasady-pracy`), bo `memory-sync.sh` jest lustrem i skasowałby je z repo.

**Why:** cała dotychczasowa pamięć i raporty (status.md, raport 27) zakładają laptopa z NVIDIA.
**How to apply:** przed testami sprzętowymi / `verify sprzet` ustal z właścicielem, czy ta maszyna jest docelowa; nie porównuj czasów z laptopem.
