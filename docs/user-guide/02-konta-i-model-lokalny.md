# Konta, klucze i model lokalny

## Alfa bez kluczy — model lokalny

Bez żadnego klucza Alfa myśli modelem uruchomionym na Twoim komputerze: **Bielik 4.5B v3.0 Instruct** (polski model,
plik Q8_0 ok. 4,8 GB z oficjalnego repozytorium autorów). W **Ustawienia → Modele lokalne i silniki** jest też lżejszy
**Bielik 1.5B v3.0 Instruct** (ok. 1,6 GB) — szybszy na słabszych kartach (np. 6 GB), ale prostszy w rozmowie i bez
zadań na plikach. Działa też bez internetu.

1. W kroku **Konta i klucze** wprowadzenia kliknij na karcie **Model lokalny** przycisk „Pobierz”. Pobieranie można
   anulować i wznowić później — nie zaczyna się od zera. Model nie ma jeszcze przypiętej sumy kontrolnej, więc po
   pobraniu zobaczysz jego sumę SHA-256 i licencję — kliknij „Ufam temu plikowi — zainstaluj”. Jeśli pominąłeś ten
   krok: **Ustawienia → Modele lokalne i silniki** (zobacz [Modele lokalne i silniki](10-modele-i-silniki.md)).
2. Program `llama-server` (llama.cpp) pobierzesz tam samo: pozycja **llama-server (vulkan)** dla kart graficznych
   AMD/Intel/NVIDIA albo **llama-server (cpu)**. Wersja programu jest jeszcze **do potwierdzenia** — jeśli pobranie
   się nie uda, skopiuj `llama-server.exe` z bibliotekami do `%LOCALAPPDATA%\Alfa\sidecars\llama-vulkan\` (albo
   `llama-cuda\`, `llama-cpu\`, wspólnie `llama\`) — Alfa wybierze właściwą kompilację. Bez tego programu pierwsza
   odpowiedź zakończy się czytelnym błędem.

Model startuje dopiero przy pierwszym pytaniu i zwalnia pamięć, gdy długo nie jest używany. Ustawienie **Urządzenia →
Tryb gry / pełnego ekranu** pozwala Alfie oddawać pamięć karty graficznej, gdy grasz albo oglądasz coś na pełnym
ekranie. Na stronie **Urządzenia** zobaczysz też wynik pomiaru sprzętu i zalecany profil („Zmierz ponownie”).

## Profil modelu

Profil decyduje, gdzie Alfa szuka odpowiedzi:

| Profil             | Działanie                                                    |
| ------------------ | ------------------------------------------------------------ |
| Lokalny            | Tylko model na Twoim komputerze.                             |
| Hybryda (domyślny) | Model lokalny i modele w chmurze — tam, gdzie lepiej pasują. |
| Chmura             | Modele dostawców w chmurze. **Wymaga klucza.**               |

Profil zmienisz chipem w polu wiadomości, komendą `/model` albo w **Ustawienia → Konta i klucze API → Domyślny profil
modelu**. Bez kluczy Alfa zawsze używa modelu lokalnego. Jeśli dostawca nie odpowiada, Alfa w ciągu 2 sekund
przełącza się na inną trasę i nie gubi wiadomości. Przy każdej odpowiedzi widzisz, który model odpowiedział
(kapsuła „Odpowiada X · model”) — wpis o wyborze trafia też na Oś czasu.

## Dodawanie konta i klucza

**Ustawienia → Konta i klucze API → Dodaj dostawcę.** Kreator ma 6 kroków:

1. **Dostawca** — z listy: Anthropic (Claude), OpenAI, Azure OpenAI, Google (Gemini), xAI (Grok), Mistral, DeepSeek,
   Kimi, Qwen, Z.ai, MiniMax, OpenRouter albo własny endpoint zgodny z OpenAI lub Anthropic (np. Ollama, LM Studio). Na liście są
   też usługi mowy (ElevenLabs, Azure Speech, Cartesia, Soniox) — ich klucze zapiszesz, ale głos w chmurze jest
   **jeszcze niedostępny**.
2. **Klucz** — wklej klucz API (przy własnym endpoincie także adres; przy koncie firmowym — zobacz niżej).
3. **Test** — Alfa sprawdza połączenie.
4. **Modele** — lista wykrytych modeli.
5. **Przypisanie** — do czego używać konta: rozmowa, kod, zadania w tle, agentki, rozpoznawanie i synteza mowy.
6. **Limit** — opcjonalny limit kosztów dla tego dostawcy (zł miesięcznie).

Klucz trafia **wyłącznie do Menedżera poświadczeń Windows** — nigdy do plików, logów ani eksportu `.alfa`; Alfa nie
pokazuje go już w interfejsie. Nowe konto działa od razu, bez restartu.

Na karcie konta widzisz stan (działa, klucz odrzucony, limit zapytań, wyłączone), liczbę modeli, ostatni test,
oznaczenie prywatności i jurysdykcji dostawcy oraz stan zgodności z regulaminem. Przyciski: **Testuj połączenie**
i **Usuń konto** (usuwa też klucz z Menedżera poświadczeń).

## Konto firmowe: organizacja i projekt OpenAI, Azure OpenAI

**Organizacja i projekt OpenAI.** Jeśli klucz należy do organizacji albo projektu (koszty i limity firmy liczą się
osobno dla każdego projektu), w kroku **Klucz** zaznacz „Klucz należy do organizacji albo projektu w OpenAI” i wpisz:

- **Identyfikator organizacji** — zaczyna się od `org-` (platform.openai.com → ustawienia organizacji),
- **Identyfikator projektu** — zaczyna się od `proj_` (ustawienia projektu).

Oba pola są opcjonalne — puste znaczy „domyślna organizacja i projekt klucza”. Alfa dołącza je do każdego zapytania
tego konta (nagłówki `OpenAI-Organization` i `OpenAI-Project`), także do testu połączenia. To nie są sekrety, ale
w dziennikach Alfa pokazuje je skrócone (`org-…1234`). Na karcie konta widzisz je w linii „Konto firmowe”.
Ten sam klucz możesz dodać kilka razy z różnymi projektami — każdy projekt to osobne konto z osobnym budżetem.

**Azure OpenAI** — dla firm, które korzystają z modeli OpenAI przez własny zasób w Microsoft Azure. Wybierz dostawcę
**Azure OpenAI** i podaj:

1. **Klucz** — z portalu Azure: zasób → **Klucze i punkt końcowy** (KEY 1 albo KEY 2). Trafia do Menedżera
   poświadczeń Windows jak każdy inny klucz; Alfa wysyła go w nagłówku `api-key`.
2. **Adres zasobu** — np. `https://firma.openai.azure.com` (sam adres, bez dalszej ścieżki; przyjmowane są też
   adresy `*.cognitiveservices.azure.com`). Jeśli wkleisz cały adres wdrożenia z portalu
   (`…/openai/deployments/…/chat/completions?api-version=…`), przycisk **Rozdziel automatycznie** wpisze osobno
   adres zasobu, nazwę wdrożenia i wersję API.
3. **Wersja API** — puste pole oznacza `2024-10-21`; wersje próbne mają końcówkę `-preview`.
4. **Wdrożenia modeli** — nazwa wdrożenia (Azure AI Foundry → Wdrożenia) i model, który udostępnia (np. wdrożenie
   `prod-4o` z modelem `gpt-4o`). Alfa wybiera model, a zapytanie trafia na adres jego wdrożenia. Bez modelu — model
   nazywa się jak wdrożenie. Co najmniej jedno wdrożenie, najwyżej 32.

Test połączenia sprawdza klucz i adres listą modeli zasobu (bez zużycia tokenów); jako modele konta pokazują się
Twoje wdrożenia. Region i zasady przetwarzania danych zależą od zasobu i umowy z Microsoft — w Alfie dostawca ma
oznaczenie prywatności „nieznane”, dopóki nie zostanie sprawdzony. Organizacji i projektu nie podaje się przy Azure,
a wdrożeń — przy innych dostawcach; kreator odmówi zapisu z wyjaśnieniem.

Test połączenia **nie sprawdza nazw wdrożeń** — literówka wyjdzie dopiero przy pierwszej rozmowie: Alfa pokaże wtedy
komunikat „Zasób Azure OpenAI nie ma wdrożenia o tej nazwie” z listą wdrożeń konta.

**Zmiana ustawień istniejącego konta** (od wersji 0.0.65): na karcie konta OpenAI albo Azure OpenAI kliknij
**Ustawienia dla firm**. Przy OpenAI zmienisz organizację i projekt (puste pole — wartość domyślna klucza), przy
Azure OpenAI — wersję API i listę wdrożeń (dodawanie, usuwanie, zmiana nazwy i modelu). Adres zasobu Azure się nie
zmienia — dla innego zasobu dodaj nowe konto. Klucza nie trzeba wklejać ponownie. Alfa sprawdza wpisy tak samo jak
przy dodawaniu konta i odmówi zapisu, gdy ten sam klucz w tym samym projekcie OpenAI ma już inne konto. Po zapisie
lista modeli konta Azure od razu pokazuje nowe wdrożenia (kolejki tracą modele, których wdrożeń już nie ma); kliknij
**Testuj połączenie**, żeby potwierdzić nowe ustawienia u dostawcy.

## Koszty i limity

- Koszt sesji widzisz w pasku tytułu; kliknięcie pokazuje koszt dzisiejszy i miesięczny oraz zużycie okna kontekstu.
- **Ustawienia → Koszty i limity**: limit miesięczny w złotych (można go całkiem wyłączyć — zostaje wtedy tylko
  wskaźnik), ostrzeżenie po przekroczeniu progu (domyślnie 80 %), „Zadania w tle tylko lokalnie” (domyślnie
  włączone) i szacowanie kosztu przed długim zadaniem.
- Ceny dostawców są w dolarach; Alfa przelicza je kursem NBP z danego dnia (gdy kursu brak — kursem zapasowym).
- Po przekroczeniu limitu odpowiedź z płatnego modelu jest blokowana z komunikatem „Przekroczono limit kosztów”.

## Klucze pod opieką: sprawdzanie w tle, wiek klucza, konto zapasowe

- **Sprawdzanie w tle.** Co 24 godziny Alfa sama sprawdza każde włączone konto — tym samym testem co przycisk
  „Testuj połączenie” (pytanie o listę modeli, bez zużycia tokenów). Gdy klucz przestanie działać, zobaczysz komunikat
  i powód przy koncie. Odstęp zmienisz w **Ustawienia → Konta i klucze API → Sprawdzanie kluczy w tle**; 0 wyłącza.
  Bez sieci nic nie jest sprawdzane.
- **Wiek klucza.** Przy koncie widać, ile dni ma zapisany klucz (liczone od dodania konta albo ostatniej wymiany).
  Po 180 dniach pojawia się przypomnienie o wymianie — próg zmienisz w **Przypomnienie o wymianie klucza**, 0 je
  wyłącza. Alfa nigdy nie wymienia klucza sama: robisz to przyciskiem „Zmień klucz”.
- **Konto zapasowe.** Dodaj drugie konto u tego samego dostawcy. Gdy pierwsze dostanie limit zapytań albo zacznie
  odrzucać zapytania (zły klucz, brak środków), kolejne wiadomości pójdą przez drugie; zobaczysz o tym komunikat.
  Wiadomość, która trafiła na błąd, ponów przyciskiem. Po limicie zapytań pierwsze konto wraca samo, gdy limit
  minie (najpóźniej po godzinie); po odrzuconym kluczu — gdy go wymienisz albo test znów przejdzie.
