# Przekazanie pracy na drugi komputer — 2026-10-19 rano

Stan po sesji na laptopie (i7-13620H, 16 GB, RTX 4050 + Intel UHD). Dalsza praca: komputer stacjonarny
(Ryzen 7 5700X3D, RX 9070 XT, 32 GB). Wszystko jest na dysku E: — `E:\PROGRAMY W CZASIE PRAC\Alfa AI Program`.

## Stan

- Repozytorium `alfa`, gałąź `ccr-af4b63c6-3fyzaj`, drzewo czyste, ostatni commit z tej sesji: patrz
  `git log -1` (opis „Przekazanie na PC”). Commity lokalne, niewypchnięte.
- **Wersja 0.0.64** (seria 5) zbudowana, sprawdzona i **wgrana** do `Alfa-uruchomiona` (19.10, 5:13, start bez błędów).
  Szczegóły: `alfa/docs/plan/29-raport-2026-10-18.md` (też w planie HTML `alfa/docs/html/`).
- Końcowy `verify wersja` 0.0.64 (commit `728558d2`): 18 z 19 etapów zielonych — Rust 2983/0, interfejs 406,
  przeglądarka 475, prawdziwa aplikacja 42/42, pokrycie łącza 88,6 %. Czerwony tylko `cargo deny`: `zip` 8.6.0
  w `updater-impl` (RUSTSEC-2026-0367/0368/0369).
- `verify sprzet` dla 0.0.64 — przerwany na polecenie właściciela (zaliczony tylko etap „nagrania”); NIE ma wyniku.
- Kopie bezpieczeństwa na dysku F: LAPTOPA („Dane”): `F:\Alfa\kopia-2026-10-19_0511` i `…_0513` (sumy zgodne).
  Na PC litera F: to inny dysk („Kopie Plików”) — `scripts/backup.sh` wymaga teraz jawnego celu i sam sprawdza dysk.

## Czeka na decyzję właściciela

1. **Gałąź `do-przegladu-updater-zip9`** — `updater-impl` na `zip` 9 + odrzucanie dowiązań DOS (AGENTS.md: zmiany
   `updater*` tylko po przeglądzie człowieka). Po akceptacji: `git merge do-przegladu-updater-zip9`,
   `cargo update -w` (root i `apps/desktop/src-tauri`), `cargo test -p updater-impl`, `cargo deny check` → zielony.
2. **Azure OpenAI** — adapter gotowy; wpis katalogu w `alfa/providers-catalog/czeka-na-wlasciciela/` (instrukcja
   w README tam): wymaga wpisu w rejestrze zgodności (propozycja: commit `b3599c6c` na gałęzi `s5-enterprise-keys`).
3. **ADR 0028** (DirectML dla Supertonic na kartach ≥ 12 GB) — podpis i potwierdzenie licencji DirectML (dane diagnostyczne).
4. Znak wodny mowy syntetycznej (C5.7) — decyzja; resztki `stare-node_modules-acl-2026-10-17` do usunięcia.

## Pierwsze kroki na PC

1. Pamięć asystenta na PC może być starsza: przed `scripts/memory-sync.sh` skopiuj zasady z
   `alfa/docs/pamiec-projektu/2-zasady-pracy/*.md` do `~/.claude/projects/E--PROGRAMY-W-CZASIE-PRAC-Alfa-AI-Program/memory/`
   (skrypt już niczego nie kasuje, ale wypisze ostrzeżenia i polecenia przywrócenia).
2. Reguły PC: `. .\scripts\portable-env.ps1`, ciężkie biegi `CARGO_BUILD_JOBS=6`, `CARGO_INCREMENTAL=0`; git z
   `-c safe.directory=*`; na PC brak Edge i `jq` (etapy przeglądarki/grafu w `verify` będą czerwone/pominięte —
   na laptopie przechodzą); `cargo fmt --all` nie działa — `rustfmt --edition 2024 <pliki>`.
3. Wtyczka „remember” — wyłączona przez właściciela (spowalniała sesję); sprawdź, czy nie działa na PC.
4. Agenci równolegli: kopie robocze tworzyć ręcznie z HEAD (`git worktree add -b <gałąź> ../wt-<seria>/<punkt> HEAD`),
   NIE opcją izolacji narzędzia (tworzyła z bardzo starego commitu `75fee0d9`). Wspólny katalog budowy agentów
   `alfa/target/agents`, jedna kompilacja naraz.

## Do zrobienia dalej

- `verify sprzet` dla 0.0.64 (na PC: karta AMD — zapytać właściciela, czy PC jest maszyną docelową tego biegu).
- Wersja 0.0.65: kolejna seria 10 punktów (roadmapa `alfa/docs/plan/03-roadmapa.md`; kandydaci: C2.8 kanwa
  artefaktów, P4 Arena / karty z profilem, P5 „Naucz”, wywołanie budżetu karty przez moduły głosu (`gpu: Some(...)`),
  C2.11a OpenRouter, porządki po 0.0.64).
- Gałęzie `s5-*` są scalone — można usunąć (`git branch -d s5-…`); zostaje `do-przegladu-updater-zip9`.
