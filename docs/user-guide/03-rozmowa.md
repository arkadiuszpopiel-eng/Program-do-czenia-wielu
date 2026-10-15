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
- **Eksportuj wiadomość do Markdown / HTML / JSON** (menu „Więcej akcji”),
- **Przetłumacz na angielski** / **na polski** — nowa prośba w tej rozmowie z samym przekładem,
- **Przypnij do tablicy** — kopia odpowiedzi trafia na **Tablicę** (do 40 przypięć z różnych rozmów; paleta
  `Ctrl+K` → **Pokaż Tablicę**: wyszukiwanie, otwórz rozmowę, kopiuj, odepnij, „Kopiuj wszystko (Markdown)”),
- **Zapisz jako szablon promptu** (Twoje wiadomości) — treść trafia do szablonów, nazwa z pierwszej linii.

**Statystyki rozmowy** (paleta `Ctrl+K` → „Statystyki rozmowy”) pokazują dla bieżącej ścieżki: liczbę wiadomości
i słów, czas czytania, tokeny, koszt odpowiedzi, średni czas odpowiedzi, użycia narzędzi, okres, inne wersje
(gałęzie po „edytuj” i „ponów”) oraz modele — z kopią jako Markdown.

Styl odpowiedzi (zwięźle, szczegółowo, prostym językiem, formalnie) i swobodę (precyzyjnie, zrównoważenie, twórczo)
ustawisz w Ustawienia → Ogólne; „Automatycznie” zostawia to personie agentki i modelowi.

Załączniki DOCX, XLSX i PPTX trafiają do modelu jako wyciągnięty tekst (akapity, slajdy, komórki arkuszy) — jak
pliki tekstowe, oznaczone jako treść z zewnątrz. Skany i PDF — w planie.

Całą rozmowę (widoczną gałąź, bez wiadomości ukrytych) wyeksportujesz z menu sesji albo z palety `Ctrl+K`:
**Eksportuj rozmowę do Markdown** / **do HTML** / **do JSON**. Plik HTML jest samodzielny: bez skryptów i zasobów
z sieci, z fontami systemowymi — można go otworzyć w przeglądarce i wydrukować do PDF. JSON służy do dalszej obróbki
(autor, rola, czas, model, treść, załączniki). Wiadomości agentek są w każdym formacie oznaczone jako wygenerowane
przez sztuczną inteligencję — czytelnie i w metadanych.

## Gałęzie: nic nie jest nadpisywane

Historia rozmowy jest tylko dopisywana. Gdy klikniesz **Ponów**, nowa odpowiedź staje obok starej — przełączasz je
strzałkami „Wariant 2 z 3”. Gdy **edytujesz** swoją wiadomość, powstaje nowa gałąź rozmowy od tego miejsca; poprzednia
zostaje i wrócisz do niej strzałkami „Gałąź 1 z 2”. **Ukryj z widoku** tylko chowa wiadomość — w dzienniku zostaje.

## Wyszukiwanie

- `Ctrl+F` — szukaj w bieżącej rozmowie (strzałki: następny, poprzedni wynik).
- `Ctrl+Shift+F` — szukaj we wszystkich sesjach (wyszukiwarka w panelu Sesje). Polskie znaki są opcjonalne:
  „zolc” znajdzie „żółć”.

## Sesje

Panel Sesje (`Ctrl+B`) ma grupy: Przypięte, Bez projektu i Archiwum. Pole **Pokaż rozmowy** zawęża listę do
dzisiejszych albo z ostatnich 7 lub 30 dni; podpowiedź przy rozmowie pokazuje liczbę wiadomości i datę założenia. Kropka przy sesji oznacza, że coś w niej
pracuje w tle; wyróżnienie — nieprzeczytane odpowiedzi. Menu sesji (prawy przycisk albo „…”):

- **Zmień nazwę** (`F2`; nazwa nadaje się też sama z pierwszych wiadomości),
- **Przypnij**, **Archiwizuj**, **Duplikuj jako szablon**,
- **Eksportuj do .alfa** — pojedyncza sesja do pliku,
- **Usuń** — przez 10 sekund możesz kliknąć „Cofnij”. Potem sesja znika na zawsze: jej baza jest zaszyfrowana,
  a klucz zostaje zniszczony.

Każda sesja ma osobną, zaszyfrowaną bazę i osobną pamięć — agentka w jednej sesji nie widzi treści innej.

## Szablony promptów

Często powtarzane prośby zapiszesz jako szablon w Ustawienia → Szablony promptów, ze zmiennymi w podwójnych
klamrach, np. „Napisz e-mail do {{adresat}} w sprawie {{temat}}”. Paleta `Ctrl+K` → **Wstaw szablon promptu** (albo
przycisk **Użyj** przy szablonie) otwiera okno: wybierasz szablon, uzupełniasz zmienne, widzisz podgląd i wstawiasz
tekst do pola wpisywania. Nic nie wysyła się samo — sprawdzasz i wysyłasz. Puste zmienne zostają w tekście
do poprawienia. **Kopiuj jako JSON** i **Wklej z JSON…** przenoszą szablony (i własne polecenia) między
komputerami albo do kopii — wklejone pozycje dopisują się do listy po sprawdzeniu.

## Polecenia bez modelu

Na proste polecenia Alfa odpowiada sama, od razu, bez modelu i bez kosztu — także bez internetu:

- **rachunki** — „Ile to jest 7 razy 8?”, „oblicz (2+3)*4”, „20% z 150”, „pierwiastek z 81”, także liczby słowami;
  procenty: „Ile procent to 25 z 200?”, „O ile procent wzrosło z 80 do 100?”;
- **literowanie** — „Przeliteruj rower”, „jak się literuje słowo żółw”;
- **losowanie** — „Rzuć kostką”, „orzeł czy reszka?”, „wylosuj liczbę od 1 do 10”, „wybierz jedno: pizza, sushi,
  żurek” (2–20 pozycji po przecinku, „albo” albo „lub”);
- **data i godzina** — „Która godzina?”, „Jaki dziś dzień?” (z zegara komputera);
- **rachunki na datach** — „Ile dni do 24 grudnia?”, „Ile dni minęło od 1 września?”, „Jaki dzień tygodnia był
  1 stycznia 2000?”, „Jaka data będzie za 30 dni?”, „Jaki dzień będzie jutro?”; data bez roku to najbliższa
  w przód (do) albo w tył (od); także „Czy 2028 jest rokiem przestępnym?” i „Ile dni ma luty 2028?”;
- **przeliczanie jednostek** — „Ile to 5 km w milach?”, „Zamień 30 stopni Celsjusza na Fahrenheita”, „Ile cm ma
  cal?”, „100 km/h na m/s”: długość, masa, objętość, temperatura, prędkość i czas (jednostki z różnych grup trafią
  do modelu);
- **liczby słownie i rzymskie** — „1234 słownie”, „Zapisz 1999,99 zł słownie” (jak na fakturze: „… złotych
  99/100”), „2026 rzymskimi”, „rzymskie MCMXC”;
- **quiz z tabliczki mnożenia** — „Przepytaj mnie z tabliczki mnożenia (przez 7)”: 10 pytań, odpowiadasz liczbą
  (także słowami albo głosem), „koniec” przerywa z podsumowaniem; inna wiadomość kończy quiz.

Polecenie musi być całą wiadomością — „ile kosztuje 5 jabłek” czy „która godzina w Tokio” trafią do modelu.

**Własne polecenia** dodasz w Ustawienia → Polecenia bez modelu: fraza → stała odpowiedź, np. „dobranoc” →
„Dobranoc! Do jutra.”. Dopasowanie: cała wiadomość, początek (dalszy tekst wstawisz jako `{reszta}`, np. „Zanotuj:
kupić mleko” → „Zapisuję: kupić mleko”), fraza w środku albo wyrażenie regularne. Tester na tej stronie pokazuje,
kto odpowie na wpisaną wiadomość — własne polecenie, wbudowane, quiz czy model. Tam też wyłączysz polecenia
bez modelu.

## Brak internetu

Gdy nie ma połączenia, wiadomość do modelu w chmurze czeka w kolejce („W kolejce — wyślę po odzyskaniu
połączenia”), a model lokalny odpowiada dalej. Polecenia bez modelu (rachunki, data, losowanie) działają od razu. Baner u góry ma przycisk „Ponów teraz”.

## Panele Pliki i Oś czasu

- **Pliki** (`Alt+3`) — pliki oddane przez agentki w tej sesji, z wersjami i podglądem. Akcje: Otwórz, Pokaż
  w Eksploratorze, Kopiuj plik, Zapisz jako.
- **Oś czasu** (`Alt+2`) — zdarzenia sesji (modele, narzędzia, audyt, głos, diagnostyka) z filtrami rodzaju
  i poziomu. Przełącznik **Replay** pokazuje pracę agentek krok po kroku (zobacz
  [Agentki i zadania](05-agentki-i-zadania.md)).
