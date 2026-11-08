---
name: alfa-silnik-nextgo
description: Silnik gier NextGO.Gray właściciela — katalog tylko do odczytu (pracuje w nim inny model); Alfa łączy się modułem tools-engine
metadata:
  node_type: memory
  type: project
  originSessionId: 526ab23b-611a-489a-a8a3-7c1153e69825
  modified: 2026-10-27T15:31:50.079Z
---

Katalog `E:\PROGRAMY W CZASIE PRAC\Silnik Do Gier AI NextGO.Gray` (nazwa tymczasowa) to drugi program właściciela, rozwijany przez inny model. **Nic w nim nie zmieniać**: nie edytować plików, nie uruchamiać `record_phase*_fixture` ani `Tools\backup_project.ps1`, nie używać portu 9001, nie zabijać jego procesów cargo.

Alfa łączy się z nim od 0.0.74 (27.10) modułem `tools-engine` (SPEC w `alfa/docs/modules/tools-engine/SPEC.md`, raport 40). Katalog wskazuje ustawienie `engine.nextgo.root`. Test sprzętowy: `ALFA_NEXTGO_ROOT=<katalog> ALFA_NEXTGO_GPU=1 cargo test -p tools-engine-impl --test engine`.

**Why:** właściciel 27.10: „w nim masz nic nie zmieniać bo nad nim pracuje inny model”.
**How to apply:** silnik tylko czytać i uruchamiać jego zbudowany serwer z `--listen 127.0.0.1:0` i danymi poza jego katalogiem; nowe metody silnika (np. zapis świata, etap 6) dodawać po stronie Alfy po przeczytaniu jego dokumentacji. Zob. [[alfa-zasady-pracy]].
