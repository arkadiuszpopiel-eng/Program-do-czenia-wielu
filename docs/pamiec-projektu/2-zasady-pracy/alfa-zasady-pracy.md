---
name: alfa-zasady-pracy
description: "Alfa — decyzje właściciela o sposobie pracy (Codex nie recenzuje, najpierw porządek, wysoki poziom katalogowania)"
metadata:
  node_type: memory
  type: feedback
  originSessionId: 526ab23b-611a-489a-a8a3-7c1153e69825
  modified: 2026-10-09T09:05:07.533Z
---

Decyzje właściciela (Arkadiusz) z 2026-10-07/08, w odpowiedzi na plan kontynuacji:

- **Codex / GPT-6 Sol nie recenzuje kodu Alfy** — zapis o przeglądzie krzyżowym w dokumentach nazwał wprost błędem.
  Zapisane w ADR 0016; recenzja = osobna sesja Claude w świeżym kontekście. Mosty do Codex CLI jako funkcja
  programu to osobna sprawa i zostają.
- **Najpierw porządek, potem rozwój**: „porządkowanie, segregacja i katalogowanie na najwyższym poziomie musi być”,
  obecny etap ma dojść do poziomu „Pro+” zanim ruszą nowe funkcje.
- Pracuje na przenośnym dysku E: — patrz [[alfa-srodowisko-lokalne]].
- **Każda seria poprawek = wyższy numer wersji z opisem poprawek** (2026-10-09, po pytaniu, czemu „O programie”
  nadal pokazuje 0.0.1). Numer zmienia skrypt `node scripts/set-version.mjs <numer>` (pliki powłoki, interfejsu i manifesty modułów), wpis w
  `CHANGELOG.md` po ludzku; po podbiciu przebudować aplikację z `ALFA_BUILD_DATE` i `ALFA_BUILD_COMMIT`, żeby okno
  pokazywało, z jakiego stanu kodu pochodzi. Wybór 0.0.2 (poprawki → ostatnia cyfra) to moja interpretacja.
- **Plan v6 prowadzony na bieżąco w MD i HTML** (polecenie właściciela z 2026-10-09: „mają być na bieżąco prowadzone
  i aktualizowane w czasie prac, po wykonaniu danego etapu”). Po zadaniu: stan w `alfa/docs/plan/03-roadmapa.md`; po
  etapie: `00-stan-wykonania.md`, wyższa wersja + CHANGELOG, `node scripts/docs-html.mjs` (HTML w `alfa/docs/html/`
  jest generowany). Nowe funkcje w planie tylko oparte na technologii sprawdzonej na dany dzień — z etykietą pewności.
- Decyzje właściciela o wielu platformach (2026-10-09): Windows 11 pierwszy, potem panel WWW, Android 15+, Linux;
  przekaźnik + E2E; zgody zdalne dla wszystkich poziomów ryzyka; telefon/WWW jako pilot 100 %, bez komputera rozmowa
  przez model API, model lokalny tylko na komputerze. Kod dopiero po podpisaniu ADR 0017–0024.
- Po każdej serii zmian uruchamiać także `cargo test --workspace -j 3`, nie tylko testy dotkniętych crate'ów
  (2026-10-09: podbicie wersji przez trzy wydania psuło testy manifestów i nikt tego nie widział). Przegląd własnych
  zmian zlecać osobnym sesjom w świeżym kontekście (ADR 0016) — przegląd 0.0.1–0.0.5 znalazł 6 średnich błędów,
  w tym dwie regresje po moich poprawkach.

**Why:** projekt powstał bardzo szybko i wszerz; właściciel nie programuje i potrzebuje, żeby stan był czytelny
z dokumentów i katalogu, a nie z kodu.

**How to apply:** każdą sesję kończyć aktualnym `docs/STATUS.md`, `docs/CATALOG.md` (generowany) i `memory/`;
nowe dokumenty wpisywać do `docs/README.md`; nie proponować recenzji przez Codexa. Moja interpretacja (nie słowa
właściciela): „Pro+” = komplet bramek zielony jednym poleceniem, zero niespójności w katalogu, dokumenty zgodne
z kodem.
