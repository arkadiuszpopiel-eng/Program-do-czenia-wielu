# 12. Kamery

Alfa obsługuje kamery wbudowane i podłączone przez USB — kilka naraz. Kamery sieciowe (IP, Wi-Fi), rozpoznawanie
osób i samodzielne patrzenie agentek są **jeszcze niedostępne** (plan: [`../plan/12-kamery-i-rozpoznawanie-osob.md`](../plan/12-kamery-i-rozpoznawanie-osob.md)).

> **Stan:** ta część nie była jeszcze sprawdzona na prawdziwej kamerze (wersja 0.0.17). Jeśli obraz jest odwrócony,
> ma złe kolory albo go nie ma — zgłoś to.

## Włączenie

1. Otwórz **Ustawienia → Kamery** (`Ctrl+,`, grupa „Głos i kamery”).
2. Włącz **„Kamery włączone”**. To przełącznik główny: dopóki jest wyłączony, żadna kamera się nie otworzy.
3. Wybierz **„Szukaj kamer”**, jeśli lista jest pusta albo podłączyłeś kamerę przed chwilą.

Każda kamera ma własny przełącznik i nazwę, którą możesz zmienić (ołówek obok nazwy) — np. „Biurko”, „Kuchnia”.

## Podgląd

**„Podgląd”** przy kamerze pokazuje obraz na żywo; możesz oglądać kilka kamer jednocześnie. Kamera otwiera się
dopiero na żądanie i zasypia po 20 sekundach bez podglądu. Podgląd niczego nie zapisuje.

| Stan kamery | Znaczenie |
| --- | --- |
| uśpiona | zamknięta; otworzy się na żądanie |
| otwieram… / zamykam… | kamera właśnie się włącza albo wyłącza |
| działa | dostarcza obraz |
| błąd | nie udało się otworzyć (np. używa jej inny program); kolejna próba ponawia |
| odłączona | system jej nie widzi |

## Znak kamery w belce

Gdy którakolwiek kamera się otwiera, działa albo jeszcze się zamyka, w górnej belce okna świeci czerwony znak kamery
(z liczbą kamer). Nie da się go ukryć. Klik prowadzi do ustawień kamer.

## „Opisz mi to, co widzisz”

Przy włączonych kamerach napisz w rozmowie:

- **„Opisz mi to, co widzisz”** — Alfa dołączy do wiadomości zdjęcie z pierwszej włączonej kamery,
- **„Co widzisz na kamerze Kuchnia?”** — zdjęcie z kamery o tej nazwie.

Pierwszy Enter tylko dołącza zdjęcie — widzisz je przy polu pisania i możesz je usunąć; **drugi Enter wysyła**
wiadomość. Zwykłe zdania, w których pada „widzisz” (np. „co widzisz w tym kodzie?”), zdjęcia nie robią.

Możesz też kliknąć **ikonę aparatu** w polu pisania i sam wybrać kamerę. Zdjęcie jest zwykłym załącznikiem: widać je
przy wiadomości i zostaje w rozmowie.

**Wymaga modelu, który przyjmuje obrazy** (np. przez konto API). Jeśli profil sesji to „Hybryda” albo „Chmura”,
zdjęcie trafia do dostawcy modelu — tak jak każdy inny załącznik. W sesji z profilem „Lokalny” zostaje na komputerze,
ale model lokalny musi obsługiwać obrazy.

Automatyczne dołączanie zdjęcia wyłączysz w **Ustawienia → Kamery → „Opisz, co widzisz” robi zdjęcie**.
W rozmowie głosowej ta funkcja jest **jeszcze niedostępna**.

## Zatrzymanie

- Przełącznik główny w „Kamery” zamyka wszystkie kamery.
- **STOP WSZYSTKIEGO** (`Ctrl+Shift+F12`) także zamyka kamery (przełączników nie zmienia).

## Prywatność

- Kamery są domyślnie wyłączone.
- Alfa nie nagrywa. Na dysk trafia wyłącznie zdjęcie, które dołączasz do wiadomości.
- Agentki nie mogą same otworzyć kamery.
