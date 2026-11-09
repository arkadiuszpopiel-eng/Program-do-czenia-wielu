# Bezpośrednia regresja Jarvisa WebView2, #50

Kandydat obejmuje wyłącznie testy i dokumentację; produkcja UI/Rust jest identyczna z d18d9433.
Dotychczasowa próba generalna sprawdzała ogólne okno i kłódkę, ale nie miała bezpośredniego
scenariusza wspólnego Jarvisa w Kreatorze i indywidualnych profili czterech wbudowanych person.

## Zakres

Wspólny przepływ UI ustawia Jarvisa w Kreatorze, otwiera Głos, przełącza Alfę, Betę, Gamę i Deltę
między Jarvisem a Standardem, przywraca ich pierwotne wybory oraz wspólny profil. Obejmuje
powrót z Ustawień, szufladę i panel dokowany. Native-wrapper odczytuje dodatkowo rzeczywiste
`voice_status` i globalne uprawnienia przed/po, wymaga ich zachowania i przywraca jedynie pięć
jawnych kluczy profilu nawet po porażce. Nie wysyła tury modelowi i nie włącza mikrofonu.

## Sprawdzenie kandydata

**2/2 Chromium w8,4s** na dotychczasowym pakiecie d18d9433:1028×779 i1600×779,
bez konkurujących obciążeń. Typy Svelte0/0, ESLint i format końcowe zaliczone.
Pierwszy wrapper odrzucił ESLint `no-unsafe-finally`; teraz cały cleanup wykonuje się poza finally,
a pierwotna porażka jest zgłaszana po sprzątaniu. Konfiguracji lintów nie zmieniono.

To test wspólnego przepływu na atrapie backendu, **nie wykonanie native-wrappera na Windows**.
Native-wrapper i jego asercje IPC wymagają następnego Windows CI;120s,15s, polityki i progi evals
pozostają dotychczasowe. Nowy zestaw zawiera903 headless E2E i68 native E2E, ale nie są to jeszcze
zaliczone wyniki pełnych zestawów. Publikacja czeka na ukończenie CI162 i czas do07:00UTC.

Logi Cloud: `/workspace/scratch/jarvis-native-{check-final,flow}.log`;
pierwszy lint opisano w `/workspace/scratch/jarvis-native-first-lint.txt`.
Nie dotykano produkcyjnych źródeł, modułów chronionych, kontraktów ani ustawień właściciela.
