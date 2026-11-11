# Zegar testu kolejki przy aktywnym zadaniu blokującym

Karta [#39](https://github.com/arkadiuszpopiel-eng/Program-do-czenia-wielu/issues/39).
Poprawka testów `app-core`; kod produkcyjny i300ms okna Cofnij pozostają dotychczasowe.

## Nieukończony Windows

CI156 na `7752ac70` po publikacji `74ce7f99` został automatycznie anulowany przez mechanizm
współbieżności workflow. Przed anulowaniem Windows utknął w dodanym w #34 teście
`queued_message_is_removed_when_the_undo_window_expires`: start celu00:30:26 UTC, komunikat
o przekroczeniu60 sekund00:31:26, brak zakończenia aż do anulowania01:29:03.

To **regresja testu dodanego w tej serii**, mimo wcześniejszych zielonych prób Linux.
Nie uznano Windows za zaliczony ani za pełny wynik. [Artefakt częściowy](artifacts/ci156-windows-incomplete.json):
139 zakończonych wyników celów,544 zaliczone /3 nieudane /7 pominiętych; osobno3 indywidualne
zaliczone testy niedokończonego celu `review`. Pozostałe cele nie zostały wykonane.
Trzy zaobserwowane porażki to2 `conductor_handoff` i brak Brokera w `files`. Późniejszych celów OEM
nie wolno dopisywać jako porażek tego przebiegu na podstawie starszego CI149.

## Przyczyna i odtworzenie

Test ręcznie przesuwał wstrzymany zegar do299ms, a potem czekał na `sleep_until` terminu300ms.
Zakładał automatyczne przesunięcie ostatniej1ms przez Tokio. Aktywne zadanie `spawn_blocking`
wyłącza to automatyczne przesunięcie; porty systemowe mogą utrzymywać takie zadania.

Dodano do tego samego testu blokującą atrapę, z potwierdzeniem startu przez `oneshot`.
Przed poprawką binarka na Linux nie kończyła testu; zewnętrzny watchdog30 sekund zakończył ją
kodem124. Nie jest to zaliczony test. Po poprawce ten sam watchdog pozostał30 sekund,
a test przeszedł w0,32s. Blokująca atrapa ma kanał zwalniany przy końcu zakresu i jest dołączana
przed zakończeniem poprawnego przebiegu; nie zostawia aktywnego procesu.

Poprawka ręcznie przesuwa ostatnią1ms do tego samego terminu300ms, następnie wznawia zegar
przed oczekiwaniem obsługi timera. Zachowuje asercję1 wiadomości przy299ms oraz wszystkie
asercje po terminie:0 w kolejce, brak Cofnij, brak sesji i brak wywołania modelu.
Nie zmieniono300ms, progów evali ani timeoutów produktu.

## Uchwyt Brokera w teście załączników

Test `attachment_taint_reaches_broker_and_survives_restart` zachowywał stare `Arc<Broker>`
po `drop(core)`, a kolejne wiązania je zasłaniały. Przed każdym kolejnym startem jawnie
zwalnia teraz stare uchwyty Brokera i powłoki. Nadal sprawdza rzeczywiste skażenie i regułę
`TaintedEgress` przed restartem, po otwarciu rozmowy i przy wysłaniu tury bez otwarcia.
Przy braku Brokera wypisuje stan i powód modułu zamiast samego `None`.

Linux zalicza scenariusz; **skuteczność poprawki uchwytów na Windows wymaga nowego CI**.
Nie przesądzono przyczyny wszystkich dawnych porażek Brokera i nie zmieniono jego implementacji.

## Wyniki

- `review`4/4, `files`3/3, `accounts_shared_queue`3/3, `review_queue`1/1: **11/11**.
- Dziesięć powtórzeń pary testów kolejki: **20 zaliczonych, zero błędów**; przypadek wygasania
  zawsze ma aktywne zadanie blokujące. Każdą parę chronił ten sam zewnętrzny watchdog30s.
- Clippy lib i wskazanych celów: zaliczony, zero ostrzeżeń. Format zaliczony.
  Rozmiary:3485 plików,264 crate'y,11 dotychczasowych wyjątków,0 naruszeń.
  HTML wygenerowany i zweryfikowany:100 stron,0 problemów; plan178 zadań,38% według istniejących stanów.

Zmienne Cargo: `CARGO_TARGET_DIR=/workspace/target-nightly`, `CARGO_PROFILE_TEST_DEBUG=0`,
`CARGO_PROFILE_DEV_DEBUG=0`, `CARGO_INCREMENTAL=0`.

```text
cargo test --locked --no-fail-fast -p app-core --test review --test files --test review_queue --test accounts_shared_queue -j2
cargo clippy --locked -p app-core --lib --test review --test files --test review_queue --test accounts_shared_queue -j2 -- -D warnings
```

Logi: `/workspace/scratch/review-blocking-{red,green,targeted,stress,clippy}.log`;
`review-blocking-status.json` zachowuje polecenia i czasy reproduktora.
Niezależny przegląd i Windows pozostają otwarte; wersja0.0.85, etap J w toku.
