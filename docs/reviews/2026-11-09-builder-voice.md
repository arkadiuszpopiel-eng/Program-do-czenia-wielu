# Kreator → Głos, #49

Przy wspólnym Jarvisie w bibliotece Kreatora dostępny jest przycisk **Sterowanie głosem**
(**Voice controls** wEN). Zamyka Ustawienia i otwiera panel Głos. Profil pozostaje zapisany;
nowa kontrolka nie zmienia mikrofonu, uprawnień, obsady ani osobnych profili wbudowanych person.
W Standardzie nie jest pokazywana, w trakcie zapisu jest nieaktywna. Właściciel osobno włącza rozmowę.

## Sprawdzenie lokalne

- RED na dotychczasowym pakiecie UI:1 nieudany przypadek classic/light, brak nowego przycisku
  przy wymaganej widoczności w5000ms. Pozostały przebieg docierał do ustawionego Jarvisa.
- Po zmianie **9/9 E2E w26,8s**:4 style×2 motywy orazEN, wyjście z Ustawień do panelu Głos,
  mikrofon nadal wyłączony, wspólny profil zachowany, osobny Standard Alfy nadal nadrzędny.
- **573/573 testy jednostkowe**,103 pliki,28,34s. Typy Svelte0 błędów/0 ostrzeżeń, ESLint i format zaliczone.
- Build20,25s; budżety startowe:mainJS134,8/150KB iCSS13,9/30KB; quick39,2/40KB; pill8,0/8KB.
  Kontrolka mieści się w leniwym widoku Ustawień; pakiet startowy pozostaje identyczny.
- Rozmiary3491 plików/264 crate'y/11 dotychczasowych wyjątków,0 naruszeń.

To Linux/headless Chromium z atrapą backendu, bez fizycznego mikrofonu.
Pełne901/901 CI161 dotyczą poprzedniej wersji12d44af4. Dla tego przycisku pełny nowy Chromium
pozostaje do wykonania w następnym CI po publikacji. Nie uruchomiono drugiej pełnej próby lokalnej;
przygotowany skrypt pozostaje niewykorzystany. Windows/WebView2 nowej kontrolki jest także otwarty.

Logi Cloud: `/workspace/scratch/builder-voice-{red,green,unit,check,build}.log`.
Chronione moduły, kontrakty, ustawienia mikrofonu i progi evals dotychczasowe.
