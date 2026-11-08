---
name: alfa-kopia-zapasowa-miejsce
description: "Kopie zapasowe Alfy od 29.10 do F:\\Kopie zapasowe programów Spopielony\\Alfa (katalog kopia-<data>), po każdym wgraniu"
metadata:
  node_type: memory
  type: feedback
  originSessionId: 1d67eb46-2bd8-4b76-bbc9-6a5dcd27cf28
  modified: 2026-10-29T16:11:32.087Z
---

2026-10-29 właściciel: „Po wgraniu proszę o zrobienie kopii zapasowej na dysku F: w Kopie zapasowe programów
Spopielony i tam odpowiedni katalog z datą”. Dawny `F:\Alfa` przeniósł do
`F:\Kopie zapasowe programów Spopielony\Alfa` (obok kopii innych projektów: NextGO.Gray, Exodus, strona).

**Why:** jedno miejsce na kopie wszystkich programów właściciela.

**How to apply:** po wgraniu wersji (`deploy-owner.sh`) uruchomić
`scripts/backup.sh "/f/Kopie zapasowe programów Spopielony/Alfa"` — skrypt sam tworzy `kopia-<data_godzina>`.
Litera F: różni się między PC i laptopem ([[maszyna-stacjonarna-2026-10-17]]) — najpierw sprawdzić, że ten
katalog istnieje. Zob. [[awaria-2026-10-17-obciazenie]].

**Od 2026-10-31 kopia ma być czysta** (właściciel: „bez zbędnych starych plików i kompilacji, tylko aktualne pliki
i kod”): `scripts/backup.sh --czysta "/f/Kopie zapasowe programów Spopielony/Alfa"`. Bez `Alfa-uruchomiona` i bez
ścieżek ignorowanych przez git (dist, test-results, gen, storybook-static); zostają paczka git, kod, pamięć i plany
(ok. 70 MB). Pierwsza: kopia-2026-10-31_1950 (7d0deb6a).

