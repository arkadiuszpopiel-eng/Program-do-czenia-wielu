# Bezpieczeństwo Alfy — zgłaszanie podatności

Alfa działa na komputerze użytkownika z dostępem do plików, poleceń, przeglądarki i kont API, więc każdą
podatność traktujemy poważnie. Ten plik mówi, jak ją zgłosić, co się dzieje dalej i w jakim czasie
(Cyber Resilience Act, art. 14 — obowiązki od 2026-09-11; plan: [`docs/plan/10-zgodnosc-prawna.md`](docs/plan/10-zgodnosc-prawna.md)).

## Jak zgłosić

- **Nie zgłaszaj podatności publicznie** (zgłoszenie publiczne, forum, czat) przed jej naprawą.
- Kontakt: **do uzupełnienia przez właściciela przed pierwszym wydaniem publicznym** — adres e-mail albo formularz
  zgłoszeń prywatnych. Do tego czasu program nie jest rozpowszechniany poza komputerami właściciela.
- Opisz: wersję Alfy (Ustawienia → O programie), system, kroki do odtworzenia, co udało się osiągnąć (np. zapis
  poza katalogiem roboczym, ominięcie zgody Brokera, odczyt sekretu) i — jeśli możesz — dowód koncepcji.
- Nie dołączaj prawdziwych kluczy API, haseł ani danych osobowych.

## Co dzieje się dalej

| Krok                                                         | Termin                                       |
| ------------------------------------------------------------ | -------------------------------------------- |
| Potwierdzenie przyjęcia zgłoszenia                           | do 3 dni roboczych                           |
| Ocena (zakres, wpływ, waga)                                  | do 10 dni roboczych                          |
| Poprawka w wersji z wpisem `[krytyczne]` w `CHANGELOG.md`    | według wagi — krytyczne priorytetowo         |
| Podatność aktywnie wykorzystywana: wczesne ostrzeżenie (CRA) | 24 h od wykrycia — CSIRT koordynator i ENISA |
| Zgłoszenie podatności (CRA)                                  | 72 h                                         |
| Raport końcowy (CRA)                                         | 14 dni od udostępnienia poprawki             |
| Ujawnienie skoordynowane ze zgłaszającym                     | po poprawce, najpóźniej 90 dni od zgłoszenia |

## Zakres

Szczególnie interesują nas: ominięcie Brokera albo poziomu autonomii, wykonanie polecenia bez zgody, zapis poza
katalogiem roboczym mimo izolacji, wyciek sekretów (Menedżer poświadczeń, tokeny CLI), wstrzyknięcie poleceń przez
treść niezaufaną (strony, pliki, poczta), XSS w renderowaniu Markdown, nadużycie łącza IPC, podmiana aktualizacji
albo modeli (sumy SHA-256), dostęp do sesji prywatnej przez trasy spoza polityki prywatności.

Model zagrożeń: [`docs/THREAT_MODEL.md`](docs/THREAT_MODEL.md).

## Wspierane wersje

Poprawki bezpieczeństwa trafiają do najnowszej wersji. Okres wsparcia wydań publicznych zostanie ustalony przed
pierwszym wydaniem (CRA — wymagania od 2027-12-11).

## Składniki i zależności

- SBOM każdego wydania: `node scripts/sbom.mjs` → `target/sbom/alfa-<wersja>.cdx.json` (CycloneDX 1.5; crate'y
  powłoki bez deweloperskich i produkcyjne pakiety interfejsu). Etap `sbom` w `node scripts/verify.mjs wersja`.
- Podatności i licencje zależności: `cargo deny check` (RustSec, lista licencji w `deny.toml`) i `pnpm audit` —
  oba w bramce wersji.
- Modele i silniki: karty modeli z licencją i pochodzeniem w Ustawienia → Modele lokalne i silniki; pliki
  z przypiętymi sumami SHA-256 albo zgodą na pierwsze użycie.
