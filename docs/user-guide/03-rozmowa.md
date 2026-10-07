# Rozmowa, gałęzie i sesje

## Pisanie wiadomości

- `Ctrl+N` albo „Nowa rozmowa” zaczyna sesję. Na pustym ekranie są trzy podpowiedzi na start.
- `Enter` wysyła, `Shift+Enter` dodaje nową linię (zamianę włączysz w **Ustawienia → Skróty → Enter wysyła
  wiadomość**). Niewysłany tekst zostaje zapamiętany jako szkic tej sesji.
- `↑` w pustym polu — popraw ostatnią wiadomość; `Ctrl+↑`/`Ctrl+↓` — wcześniej wysłane; `Ctrl+Shift+V` — wklej bez
  formatowania.
- **@imię** (np. `@Delta`) kieruje wiadomość do konkretnej agentki. Bez tego odpowiada Dyrygentka.
- **/komendy**: `/obsada` (zmień obsadę agentek), `/model` (profil modelu), `/eksport` (sesja do pliku `.alfa`),
  `/ustawienia`, `/skupienie`, `/nowa`.
- Chipy przy polu pokazują adresatkę i profil modelu — kliknij, żeby zmienić.
- `Esc` albo „Stop” zatrzymuje odpowiedź w trakcie.
- **Załączniki**: spinacz przy polu (okno wyboru plików), wklejenie pliku albo obrazu (`Ctrl+V`) albo przeciągnięcie
  plików na okno. Alfa kopiuje je do katalogu sesji (`…\Alfa\Sesje\<nazwa>\in`) i pokazuje jako chipy z rozmiarem
  i szacunkiem tokenów; przy dużych załącznikach — także udziałem w oknie kontekstu i szacunkiem kosztu. Limity:
  10 plików w wiadomości, 25 MB na plik, 100 MB razem. Obraz do 5 MB model widzi w całości, tekst (do 100 000 znaków)
  jako dane, nie polecenia; z innych plików (PDF, archiwa) model dostaje tylko nazwę, typ i rozmiar. Pliki z danych
  Alfy i katalogów z poświadczeniami (np. `.claude`, `.codex`, profile przeglądarek) są odrzucane. Po wysłaniu
  załączniki są w panelu **Pliki**. Wiadomość może składać się z samych załączników.
- Zrzut ekranu prosto z pola wiadomości — **jeszcze niedostępny**.

## Odpowiedź

Odpowiedź pojawia się na bieżąco. Zwinięty blok „Myślenie · czas” oznacza, że model najpierw się zastanawiał.
Kroki narzędzi (gdy agentka działa na plikach) są zwinięte do jednej linii — kliknij, żeby rozwinąć. Pod odpowiedzią
widać model, liczbę tokenów, koszt i czas.

Bloki kodu mają przyciski: **Kopiuj kod**, **Zapisz jako plik**, **Zawijaj wiersze**. Przycisk **Uruchom w
terminalu (przez Brokera)** w tej wersji nie wykonuje kodu — skopiuj go i uruchom samodzielnie.

## Akcje wiadomości

Najedź na wiadomość albo przejdź do niej klawiszem `Tab`:

- **Kopiuj** (Markdown) i **Kopiuj jako tekst**,
- **Przeczytaj na głos** — głosem agentki, która napisała odpowiedź (wymaga silnika mowy, zobacz [Głos](04-glos.md)),
- **Ponów** — nowa wersja odpowiedzi; także **Ponów modelem lokalnym** / **w chmurze**,
- **Edytuj i wyślij ponownie** (Twoje wiadomości),
- **Kontynuuj** — dokończ odpowiedź uciętą limitem długości,
- **Zapamiętaj** — w tej sesji, w projekcie, globalnie albo u agentki (zobacz [Pamięć](06-pamiec.md)),
- ocena „Dobra odpowiedź” / „Słaba odpowiedź”, **Szczegóły → Oś czasu**, **Ukryj z widoku**,
- **Eksportuj wiadomość do Markdown / HTML** (menu „Więcej akcji”).

Całą rozmowę (widoczną gałąź, bez wiadomości ukrytych) wyeksportujesz z menu sesji albo z palety `Ctrl+K`:
**Eksportuj rozmowę do Markdown** / **do HTML**. Plik HTML jest samodzielny: bez skryptów i zasobów z sieci, z fontami
systemowymi — można go otworzyć w przeglądarce i wydrukować do PDF.

## Gałęzie: nic nie jest nadpisywane

Historia rozmowy jest tylko dopisywana. Gdy klikniesz **Ponów**, nowa odpowiedź staje obok starej — przełączasz je
strzałkami „Wariant 2 z 3”. Gdy **edytujesz** swoją wiadomość, powstaje nowa gałąź rozmowy od tego miejsca; poprzednia
zostaje i wrócisz do niej strzałkami „Gałąź 1 z 2”. **Ukryj z widoku** tylko chowa wiadomość — w dzienniku zostaje.

## Wyszukiwanie

- `Ctrl+F` — szukaj w bieżącej rozmowie (strzałki: następny, poprzedni wynik).
- `Ctrl+Shift+F` — szukaj we wszystkich sesjach (wyszukiwarka w panelu Sesje). Polskie znaki są opcjonalne:
  „zolc” znajdzie „żółć”.

## Sesje

Panel Sesje (`Ctrl+B`) ma grupy: Przypięte, Bez projektu i Archiwum. Kropka przy sesji oznacza, że coś w niej
pracuje w tle; wyróżnienie — nieprzeczytane odpowiedzi. Menu sesji (prawy przycisk albo „…”):

- **Zmień nazwę** (`F2`; nazwa nadaje się też sama z pierwszych wiadomości),
- **Przypnij**, **Archiwizuj**, **Duplikuj jako szablon**,
- **Eksportuj do .alfa** — pojedyncza sesja do pliku,
- **Usuń** — przez 10 sekund możesz kliknąć „Cofnij”. Potem sesja znika na zawsze: jej baza jest zaszyfrowana,
  a klucz zostaje zniszczony.

Każda sesja ma osobną, zaszyfrowaną bazę i osobną pamięć — agentka w jednej sesji nie widzi treści innej.

## Brak internetu

Gdy nie ma połączenia, wiadomość do modelu w chmurze czeka w kolejce („W kolejce — wyślę po odzyskaniu
połączenia”), a model lokalny odpowiada dalej. Baner u góry ma przycisk „Ponów teraz”.

## Panele Pliki i Oś czasu

- **Pliki** (`Alt+3`) — pliki oddane przez agentki w tej sesji, z wersjami i podglądem. Akcje: Otwórz, Pokaż
  w Eksploratorze, Kopiuj plik, Zapisz jako.
- **Oś czasu** (`Alt+2`) — zdarzenia sesji (modele, narzędzia, audyt, głos, diagnostyka) z filtrami rodzaju
  i poziomu. Przełącznik **Replay** pokazuje pracę agentek krok po kroku (zobacz
  [Agentki i zadania](05-agentki-i-zadania.md)).
