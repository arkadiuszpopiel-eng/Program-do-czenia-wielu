# Windows/WebView2:62 zaliczone, #47

[Rehearsal82](https://github.com/arkadiuszpopiel-eng/Program-do-czenia-wielu/actions/runs/42564408255),
źródło `12d44af48df812d59f9978a9b5b902011e3a6ab1`, zadanie128987662454.
Zakończone powodzeniem: **62 zaliczone,0 nieudanych,5 pominiętych**,2,5min; podsumowanie
05:09:41UTC. Poprzedni rehearsal81 miał60/1/6; nie zastępujemy jego wyniku zielonym wstecznie.

## Blokada dziecięca i modal

`zy-klodka-dziecko` przeszedł w3,0s. Wszystkie stałe kroki diagnostyczne mają start/ok:
odczyt stanu, ustawienie PIN-u, włączenie blokady, klik menu, zablokowane wyższe tryby,
próba skrótu, odmowa pełnego dostępu, otwarcie PIN-em w rozmowie A i zamknięcie po przejściu
do rozmowy B, przywrócenie rozmowy i sprzątanie. Klik menu05:09:38.739–38.812.

To potwierdza poprawkę przygotowania testu #47: pozostały modal Głosu trzeba zamknąć
zwykłym przyciskiem przed klikiem formularza. Timeout120s, polityka i asercje dotychczasowe;
bez kliknięcia force i bez wyłączania scrim. Pierwotny ChromiumRED i dwaGREEN zachowano
w [raporcie diagnozy](2026-11-09-composer-modal.md).

Dokowanie `z-night` zaliczone38,4s. Dotyczy jawnej emulacji viewportu1600×900 w tym fragmencie,
z przywróceniem metryk; nie stanowi próby natywnego szerokiego monitora właściciela.

## Pięć pominięć i zakres

- `z-pelny-dostep`: brak specjalnego `ALFA_TEST_AUTONOMY`; zwykły poziom domyślny.
- Logowanie Claude Code: brak wykrytego CLI na runnerze; nie uruchomiono mostu ani logowania.
- Natywne okno eksportu: bieg ukryty, wymaga osobnego widocznego pulpitu.
- Dyktowanie i czytanie zaznaczenia przez skróty: bieg ukryty; osobne próby widocznego pulpitu.

Te pominięcia są dotychczasowymi warunkami zestawu, nie nowym obejściem porażek.
Rzeczywista aplikacja działała na Windows/WebView2 z kompilacją testową e2e i CDP.
Nie ma dowodu fizycznego mikrofonu, odsłuchu, produkcyjnej usługi Brokera na osobnym koncie ani
odbioru na sprzęcie właściciela. Powłoka Tauri CI161 także zaliczona; pełny Rust Windows i UI
CI161 w momencie sporządzenia tego raportu jeszcze trwają.
