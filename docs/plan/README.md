# Plan Alfy — wersja 6 (rozszerzona)

Stan na 2026-10-09. Ten katalog jest **rozszerzeniem** zatwierdzonego planu [`../PLAN.md`](../PLAN.md) (wersja 5):
niczego w nim nie kasuje, tylko dopisuje nowe cele — kolejne systemy, sterowanie przez Internet i nowe funkcje —
oraz pokazuje, co z planu v5 jest już wykonane i sprawdzone.

Wersja do czytania w przeglądarce: [`../html/index.html`](../html/index.html) (otwierana dwuklikiem, bez serwera).

## Co gdzie jest

| Plik | Co zawiera | Kiedy się zmienia |
| --- | --- | --- |
| [`00-stan-wykonania.md`](00-stan-wykonania.md) | Co jest wykonane i przetestowane: wersje, fale F0–F9, różnica „✅ CI” a „✅ sprzęt” | po każdym etapie i każdej wersji |
| [`01-wizja-i-zasady.md`](01-wizja-i-zasady.md) | Po co ten plan, zasady, decyzje właściciela, co się zmienia względem v5 | przy nowej decyzji |
| [`02-architektura-docelowa.md`](02-architektura-docelowa.md) | Host i klienci, fabryka platform, protokół, mapa gotowości kodu | przy zmianie architektury (ADR) |
| [`03-roadmapa.md`](03-roadmapa.md) | **Dokument żywy**: etapy A–H, zadania, kryteria wyjścia, statusy | po każdym zadaniu |
| [`04-nowe-funkcje.md`](04-nowe-funkcje.md) | Katalog nowych funkcji z technologią, źródłem i poziomem pewności | gdy funkcja dochodzi albo zmienia stan |
| [`05-windows.md`](05-windows.md) | Windows 11 — pierwsza wersja | z postępem etapów A–C |
| [`06-linux.md`](06-linux.md) | Linux — co trzeba zbudować | z postępem etapu G |
| [`07-panel-www.md`](07-panel-www.md) | Panel WWW — sterowanie przez Internet | z postępem etapów D–E |
| [`08-android.md`](08-android.md) | Android 15 i nowsze | z postępem etapu F |
| [`09-dostep-zdalny-i-bezpieczenstwo.md`](09-dostep-zdalny-i-bezpieczenstwo.md) | Przekaźnik, parowanie, zgody zdalne, praca bez komputera, nowe zagrożenia | przy zmianie ADR 0021–0023 |
| [`10-zgodnosc-prawna.md`](10-zgodnosc-prawna.md) | AI Act, CRA, RODO, dostępność — terminy i zadania | co kwartał i przed wydaniem |
| [`11-technologie-2026-10.md`](11-technologie-2026-10.md) | Przegląd technologii ze źródłami i datą sprawdzenia | co kwartał i przed wyborem technologii |
| [`12-kamery-i-rozpoznawanie-osob.md`](12-kamery-i-rozpoznawanie-osob.md) | Kamery (USB, IP, Wi-Fi), rozpoznawanie zapisanych osób z pamięcią, opis obrazu na polecenie | z postępem grupy C3 |
| [`13-podsumowanie-i-dalsze-kroki.md`](13-podsumowanie-i-dalsze-kroki.md) | Podsumowanie zgłoszeń właściciela i ich stanu, decyzje do podjęcia, plan następnych sesji | po każdej dłuższej sesji |

Projekty decyzji do tego planu: [`../ADR/`](../ADR/README.md), numery 0017–0025 (status „Proponowany”).
Szkice specyfikacji nowych modułów: [`../modules/README.md`](../modules/README.md), sekcja „Planowane (plan v6)”.

## Zasady prowadzenia

1. **Po każdym zakończonym zadaniu** sesja zmienia jego status w [`03-roadmapa.md`](03-roadmapa.md), a jeśli
   zadanie zamyka kryterium akceptacji — także w [`../STATUS.md`](../STATUS.md) (tylko tam żyje stan kryteriów).
2. **Po każdym zakończonym etapie** sesja aktualizuje [`00-stan-wykonania.md`](00-stan-wykonania.md), podnosi
   wersję programu z wpisem w [`../../CHANGELOG.md`](../../CHANGELOG.md) i przebudowuje HTML:
   `node scripts/docs-html.mjs`.
3. **HTML jest generowany** z plików MD — nie edytujemy go ręcznie. Bramka `scripts/gates.ps1` przebudowuje go
   i sprawdza odnośniki.
4. **Nic nie jest „sprawdzone”, jeśli nie było uruchomione.** „✅ CI” znaczy: przechodzi na atrapach i w testach
   automatycznych. „✅ sprzęt” znaczy: zmierzone albo obejrzane w działającym programie na prawdziwej maszynie
   (ADR 0016).
5. **Technologia trafia do planu z etykietą pewności** (patrz [`11-technologie-2026-10.md`](11-technologie-2026-10.md)).
   Przed rozpoczęciem zadania, które od niej zależy, sesja sprawdza ją ponownie w źródle pierwotnym.
6. Zmiany decyzji właściciela, progów akceptacji i reguł bezpieczeństwa — tylko za zgodą właściciela (ADR).

## Statusy w roadmapie

Skrypt HTML liczy postęp etapów z kolumny „Stan”, dlatego używamy dokładnie tych słów:

| Stan | Znaczenie |
| --- | --- |
| `zrobione` | wykonane i sprawdzone testami; data w kolumnie obok |
| `w toku` | rozpoczęte, niezamknięte |
| `do zrobienia` | może ruszyć, nic go nie blokuje |
| `czeka` | zablokowane przez inne zadanie, decyzję albo działanie właściciela — powód w uwagach |
| `planowane` | dalszy etap; szczegóły dojdą przed jego rozpoczęciem |
| `badawcze` | bez zobowiązania — najpierw próba, potem decyzja |
