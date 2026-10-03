# Konta, klucze i model lokalny

## Alfa bez kluczy — model lokalny

Bez żadnego klucza Alfa myśli modelem uruchomionym na Twoim komputerze: **Bielik 4.5B v3.0 Instruct** (polski model,
plik ok. 2,9 GB). Działa też bez internetu.

1. W kroku **Konta i klucze** wprowadzenia kliknij na karcie **Model lokalny** przycisk „Pobierz”. Pobieranie można
   anulować i wznowić później — nie zaczyna się od zera. Jeśli pominąłeś ten krok: `Ctrl+K` → „Uruchom ponownie
   wprowadzenie”.
2. **Wymaga plików:** program `llama-server.exe` z projektu llama.cpp nie jest jeszcze pobierany automatycznie.
   Skopiuj go do `%LOCALAPPDATA%\Alfa\sidecars\llama\llama-server.exe`. Możesz trzymać osobne kompilacje dla karty
   graficznej: `sidecars\llama-vulkan\`, `sidecars\llama-cuda\`, `sidecars\llama-cpu\` — Alfa wybierze właściwą.
   Bez tego pliku pierwsza odpowiedź zakończy się czytelnym błędem.

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

Profil zmienisz chipem w polu wiadomości, komendą `/model` albo w **Ustawienia → Modele i dostawcy → Domyślny profil
modelu**. Bez kluczy Alfa zawsze używa modelu lokalnego. Jeśli dostawca nie odpowiada, Alfa w ciągu 2 sekund
przełącza się na inną trasę i nie gubi wiadomości. Przy każdej odpowiedzi widzisz, który model odpowiedział
(kapsuła „Odpowiada X · model”) — wpis o wyborze trafia też na Oś czasu.

## Dodawanie konta i klucza

**Ustawienia → Modele i dostawcy → Dodaj dostawcę.** Kreator ma 6 kroków:

1. **Dostawca** — z listy: Anthropic (Claude), OpenAI, Google (Gemini), xAI (Grok), Mistral, DeepSeek, Kimi, Qwen,
   Z.ai, MiniMax, OpenRouter albo własny endpoint zgodny z OpenAI lub Anthropic (np. Ollama, LM Studio). Na liście są
   też usługi mowy (ElevenLabs, Azure Speech, Cartesia, Soniox) — ich klucze zapiszesz, ale głos w chmurze jest
   **jeszcze niedostępny**.
2. **Klucz** — wklej klucz API (przy własnym endpoincie także adres).
3. **Test** — Alfa sprawdza połączenie.
4. **Modele** — lista wykrytych modeli.
5. **Przypisanie** — do czego używać konta: rozmowa, kod, zadania w tle, agentki, rozpoznawanie i synteza mowy.
6. **Limit** — opcjonalny limit kosztów dla tego dostawcy (zł miesięcznie).

Klucz trafia **wyłącznie do Menedżera poświadczeń Windows** — nigdy do plików, logów ani eksportu `.alfa`; Alfa nie
pokazuje go już w interfejsie. Nowe konto działa od razu, bez restartu.

Na karcie konta widzisz stan (działa, klucz odrzucony, limit zapytań, wyłączone), liczbę modeli, ostatni test,
oznaczenie prywatności i jurysdykcji dostawcy oraz stan zgodności z regulaminem. Przyciski: **Testuj połączenie**
i **Usuń konto** (usuwa też klucz z Menedżera poświadczeń).

## Koszty i limity

- Koszt sesji widzisz w pasku tytułu; kliknięcie pokazuje koszt dzisiejszy i miesięczny oraz zużycie okna kontekstu.
- **Ustawienia → Koszty i limity**: limit miesięczny w złotych (można go całkiem wyłączyć — zostaje wtedy tylko
  wskaźnik), ostrzeżenie po przekroczeniu progu (domyślnie 80 %), „Zadania w tle tylko lokalnie” (domyślnie
  włączone) i szacowanie kosztu przed długim zadaniem.
- Ceny dostawców są w dolarach; Alfa przelicza je kursem NBP z danego dnia (gdy kursu brak — kursem zapasowym).
- Po przekroczeniu limitu odpowiedź z płatnego modelu jest blokowana z komunikatem „Przekroczono limit kosztów”.
