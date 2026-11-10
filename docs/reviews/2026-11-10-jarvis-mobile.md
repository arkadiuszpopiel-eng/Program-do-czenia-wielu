# Kokpit Jarvis i przegląd mobilny — #68

Nowe polecenie właściciela, baza13c8c88d. PełneCI179/Windows101 tej bazy są zielone.
UI zmieniono w osobnym worktree, bez chronionych modułów, polityk, zależności lub progówevals.

## Audyt aktualnego ekranu

1. Desktop1440x900: działający układNext, rozmowa/panele/zgody. Zrzutbefore-desktop.png.
2. Telefon390x844: układ mieści rozmowę, ale główne przyciski mają28–32px; widoczne
   kontrolkiWindows zajmują miejsce w przeglądarce. Zrzutbefore-phone.png.
3. Mały ekran320x740: gęsta belka i composer; wymaga większych celów dotykowych i kontroli
   dostępności pisania. Zrzutbefore-small-phone.png. Pozaekranowy kod w wewnętrznym
   przewijaniu nie jest dowodem poziomego przewijania całej strony.

Wszystkie sześć zrzutów przed/po zachowano i obejrzano w scratch/jarvis-mobile.
To bieżąca emulacjaChromium, z atrapąAPI, nie fizyczny telefon albo połączenie z komputerem.

## Zmiana

Przycisk radaru w belce włącza/wyłącza lokalny kokpit Jarvis. Wybór zapamiętuje się w oknie;
nie dodaje wartości do chronionego katalogu ustawieńCore i nie zmienia trybu zgód lub person.
Kokpit ładuje się leniwie, ma radar dekoracyjny i kafelki rozmów,agentek,zadań,głosu,modeli,
uprawnień. Pokazuje faktyczny stan bieżącej obsady; radar nie udaje pomiaru.

Jasny/ciemny motyw i wszystkie cztery dotychczasowe style zachowane. Ograniczenie animacji
systemu i ustawienie aplikacji wyłączają ruch radaru. Na dotykowych ekranach do1100px główne
przyciski belki mają44px. W mobilnej przeglądarce kontrolkiWindows są ukryte; wTauri pozostają.
Kafelki układają się w3kolumny na małym ekranie,6na dużym; niski ekran pomija dekorację.

## Testy

RED:0/2 — brak przełącznika oraz cel32px zamiast44. GREEN:2/2. Rozszerzenie macierzy:
18/19; błąd selektora testu głosu szukałtab zamiast strony ustawień. Aplikacja otworzyła
poprawne ustawienia; poprawiono asercję do istniejącegoaria-current, bez zmiany zachowania.
Końcowa regresja57/57 obejmuje nowe21prób, wcześniejszegoJarvisa/kreator, karty sesji i geometrię
ustawień. Zaliczone: 4style×2motywy×2szerokości,320x568 i844x390,44px,composer,axe,
nawigacja, zapis wyboru oraz brak zmiany trybu zgód. Svelte-check0błędów/0ostrzeżeń.
Surowe komendy, wyniki, logi iSHA256 pozostają w scratch/jarvis-mobile.

Nowy head wymaga własnego pełnegoCI i niezależnego przeglądu przed merge. Odbiór nowegoUI
Windows/WebView2 oraz prawdziwego telefonu nie jest tu zaliczony. #41/#48,GPU,API,mikrofon
i wyszukiwarka pozostają otwarte. [Plan kolejnych etapów](../plan/56-kokpit-jarvis-i-panel-telefonu.md).
