# Mosty CLI (Claude Code, Codex)

Jeśli masz abonament i używasz w wierszu poleceń **Claude Code** (`claude`) albo **Codex** (`codex`), agentki mogą
zlecać im pracę — na Twoim koncie i zgodnie z regulaminem dostawcy. Alfa uruchamia wtedy oficjalny, niezmieniony
program i pokazuje postęp. **Wymaga:** zainstalowanego CLI i zalogowania się w nim.

## Logujesz się Ty

Alfa nigdy nie loguje się za Ciebie, nie czyta ani nie przechowuje tokenów CLI. Logowanie:

- w kroku **Mosty CLI** wprowadzenia albo na karcie mostu przycisk **Zaloguj w terminalu** — otwiera się wbudowany
  terminal w katalogu domowym; wpisz polecenie logowania (`claude` i `/login` albo `codex login`) i postępuj według
  instrukcji narzędzia;
- przycisk **Pokaż polecenie logowania** podaje polecenie do skopiowania — Alfa niczego sama nie wykonuje;
- gdy terminala nie da się otworzyć, otwórz go sam i wpisz to samo polecenie.

Wbudowany terminal (także **Ustawienia → Komputer → Otwórz PowerShell / wiersz polecenia**) widzisz tylko Ty: jego
treść nie trafia do agentek, zdarzeń ani logów. Zamykasz go przyciskiem „Zamknij terminal” (`Esc` trafia do programu
w terminalu).

## Karty zgodności

**Ustawienia → Modele i dostawcy → Mosty CLI** (przycisk „Wykryj ponownie” sprawdza zainstalowane wersje). Każda
trasa ma kartę:

- stan: **Zgodna**, **Niezweryfikowana** albo **Zabroniona**; nieaktualny wpis rejestru zgodności automatycznie
  obniża trasę do „niezweryfikowanej”,
- wersja wykryta i **przypięta** — most uruchomi tylko przypiętą wersję („Przypnij wykrytą wersję”; bez przypięcia
  most odmówi startu),
- data weryfikacji, regulamin i źródła (adres do skopiowania), co jest dozwolone, a co zabronione,
- **Trasa włączona** — wyłącznik; trasy zabronionej nie da się włączyć,
- **Zgoda na uruchamianie z harmonogramu** i limit uruchomień na dobę (najwyżej 24). Bez tej zgody most startuje
  tylko na Twoje bezpośrednie polecenie.

## Zlecanie pracy z rozmowy

Napisz na **początku** wiadomości, np.:

- „Delta, zleć to Claude Code” — zleca treść poprzedniej wiadomości,
- „przekaż Codexowi: popraw testy w module płatności”.

Polecenie musi stać na początku wiadomości — to samo zdanie w środku wklejonego maila niczego nie uruchomi. Most
pracuje na kopii projektu (osobny worktree), a nie w Twoim katalogu. Postęp widać w rozmowie, na Osi czasu i w panelu
Zadania z dopiskiem „wynik niezweryfikowany przez Alfę”. Prośby CLI o uprawnienia trafiają do Brokera; w tej wersji,
bez okna Brokera, kończą się odmową po upływie czasu.

Wyzwalacze zdarzeniowe (np. nowy plik), Ulepszacz i podzadania agentek **nigdy** nie uruchamiają mostów;
harmonogram — tylko po włączeniu zgody na karcie mostu.
