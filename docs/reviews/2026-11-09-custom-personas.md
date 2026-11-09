# Własne persony: Jarvis i odtworzenie katalogu

Karty [#35](https://github.com/arkadiuszpopiel-eng/Program-do-czenia-wielu/issues/35),
[#36](https://github.com/arkadiuszpopiel-eng/Program-do-czenia-wielu/issues/36).
Linux Cloud, rdzeń bez okna, model i audio na atrapach. To nie próba Windows/WebView2 ani mikrofonu.

## Problem i zmiana

Nowa regresja tworzy Lunę przez publiczny Kreator: podgląd, test na sucho tego samego hasha,
zapis i przypisanie własnej roli tylko do odczytu. Luna dziedziczy wspólnego Jarvisa, zachowuje
imię „Luna” / „Jestem Luną”, własny opis roli i zestaw narzędzi. Delegacja z Alfy w Standardzie
otrzymuje profil Luny; profil rodzica pozostaje Standardowy.

Pierwsza próba: **5 dotychczasowych testów Jarvisa zaliczonych, nowa delegacja zaliczona,
restart niezaliczony**. Po restarcie `agents_list` nie zawierała Luny. Cel głosowy nie został wtedy
uruchomiony, ponieważ Cargo zakończyło polecenie po nieudanym celu `jarvis_custom`.

Biblioteka manifestów Kreatora była odczytywana z dysku, ale wspólna usługa `personas` zaczynała
od katalogu wbudowanego. `app-skills` odtwarza teraz brakujące definicje person i ról przed
udostępnieniem Kreatora. Najpierw sprawdza cały zestaw walidacją `Catalog`, potem dodaje wpisy.
Konflikt lub wpis wbudowany nie nadpisuje katalogu. Identyczne definicje są idempotentne.
Odtworzenie nie wykonuje zadań, nie aktywuje wyzwalaczy i nie ustala poziomu Brokera.

**Obsada konkretnej rozmowy nadal nie jest trwała.** Źródła potwierdziły, że jest przechowywana
w pamięci procesu. Początkowy test zakładał również zachowanie tego przydziału. Zakres poprawki
ograniczono jawnie do odtworzenia katalogu i profilu po ponownym przypisaniu roli przez właściciela;
zmieniono też nazwę testu. [Karta #37](https://github.com/arkadiuszpopiel-eng/Program-do-czenia-wielu/issues/37)
obejmuje zapis obsady, restart, walidację oraz usunięcie metadanych przy ostatecznym usunięciu sesji.
Nie uznano aspiracyjnego wymagania trwałej obsady za zaliczone.

## Sprawdzenie

- Ukierunkowane cele: **14 zaliczonych / 0 nieudanych / 0 pominiętych**: Jarvis5, własna persona2,
  syntetyczny głos1, `app-skills`6 (w tym3 nowe przypadki odtworzenia); `app-core` lib0.
- Dotychczasowy `builder_saves_only_after_dry_run_of_the_same_hash`: **1/1**,4 odfiltrowane.
- Nowe przypadki sprawdzają brak ponownej publikacji, brak przydziału ról, odrzucenie całej partii
  przed publikacją przy późniejszym konflikcie i ochronę istniejących/wbudowanych definicji.
- Clippy lib i wskazanych celów `app-skills`/`app-core`: zaliczone, zero ostrzeżeń.
- Format i rozmiary zaliczone:3484 pliki,264 crate'y,11 dotychczasowych wyjątków,0 naruszeń.
  Graf zależności1808 krawędzi,0 naruszeń. IPC aktualne:397 typów,285 komend.
  Dokumentacja HTML:100 stron,0 problemów; plan prac wygenerowany (178 zadań,38% według istniejących stanów).
- `Cargo.lock`: wyłącznie krawędź do istniejącego `personas-fake` dla testów; bez zmiany wersji.

Komendy (wspólne zmienne: `CARGO_TARGET_DIR=/workspace/target-nightly`,
`CARGO_PROFILE_TEST_DEBUG=0`, `CARGO_PROFILE_DEV_DEBUG=0`, `CARGO_INCREMENTAL=0`):

```text
cargo test --locked --no-fail-fast -p app-skills -p app-core --lib --test jarvis_custom --test jarvis --test jarvis_voice -j2
cargo test --locked -p app-core --test computer builder_saves_only_after_dry_run_of_the_same_hash -j2
cargo clippy --locked -p app-skills -p app-core --lib --test jarvis_custom --test jarvis --test jarvis_voice --test computer -j2 -- -D warnings
```

Pierwsza próba sprawdzeń poprawki zatrzymała się na `--locked` przed testami. Uzupełniono lockfile
przez `cargo metadata --offline`; właściwy przebieg opisany wyżej korzystał już z `--locked`.
Logi: `/workspace/scratch/jarvis-custom-tests.log` (pierwsza porażka),
`builder-restore-tests-run.log`, `builder-restore-existing-test.log`, `builder-restore-clippy.log`.

## Pełny UI i osobne wyniki CI

Pełny lokalny E2E na `bd389207`, UI równoważne opublikowanemu `7752ac70`:
**877 zaliczonych / 2 nieudane / 0 pominiętych / 0 flaky**,37,8 minuty,2 workers.
[Artefakt](artifacts/ui-full-rerun-2026-11-09.json) zawiera dokładne błędy i hash surowego raportu.
Obie porażki: `plan-mode.spec.ts:30`, Glass light/dark — karta Planu nie pojawiła się w5 sekundach.
Snapshot jasny pokazuje kartę po porażce asercji, ciemny nadal generowaną odpowiedź. Diagnoza
timerów i synchronizacji funkcjonalnej pozostaje otwarta; nie podniesiono5 sekund.

Na tym samym opublikowanym źródle [CI156](https://github.com/arkadiuszpopiel-eng/Program-do-czenia-wielu/actions/runs/42537977215)
ma **879/879 E2E i573/573 jednostkowych**, lint/typy/build/bundle/Storybook zielone. To oddzielny
przebieg, nie zastępuje lokalnych porażek. Windows Rust nadal trwał przy zbieraniu tego raportu;
Linux Clippy blokuje znany chroniony import Brokera.

[WebView2 rehearsal77](https://github.com/arkadiuszpopiel-eng/Program-do-czenia-wielu/actions/runs/42537977174):
**60 zaliczonych / 2 nieudane / 5 pominiętych**,4,4 minuty, runner GitHub. Skrypt ustawia
`ALFA_E2E_INSTANCE` tylko dla portu innego niż9222, a CI używa9222 — test plakietki nie ma flagi.
Kontekst testu paneli pokazuje „Pokaż panel Sesje” i brak nawigacji Sesji; test próbuje od razu
kliknąć „Schowaj panel Sesje”. Komunikat zamkniętej strony następuje po120 sekundach oczekiwania
i sprzątaniu testu, nie stanowi dowodu awarii aplikacji. Poprawki harnessu są do wykonania.
SHA256 archiwum: `4c89a1b826eaee1d208829e1d284937993a6e00a60a5614e79ccc7190322658f`.

Wersja0.0.85, etap J otwarty. Niezależny przegląd, pełne nowe próby i odbiór sprzętowy pozostają wymagane.
