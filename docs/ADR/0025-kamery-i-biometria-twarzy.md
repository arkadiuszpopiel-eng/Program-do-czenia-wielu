# ADR 0025 — Kamery i biometria twarzy: zgoda, dane tylko lokalnie, rozpoznanie nie jest autoryzacją

| Pole      | Wartość                                                                                                                                                                                                                                              |
| --------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Status    | **Przyjęty** — zgoda właściciela z 2026-10-11 („możesz wykonać w pełni, masz zgodę”)                                                                                                                                                                                                            |
| Data      | 2026-10-09                                                                                                                                                                                                                                           |
| Powiązane | plan v6: `docs/plan/12-kamery-i-rozpoznawanie-osob.md`, `03-roadmapa.md` (C3), `10-zgodnosc-prawna.md`; SPEC: `camera-hub`, `vision-describe`, `person-recognition`, `person-memory`; ADR 0003, 0015 (Broker i dowody zgody); `docs/THREAT_MODEL.md` |
| Zmienia   | zakres `tools-media` (kamera przestaje być „narzędziem mediów”, dostaje własne moduły); dopisuje zagrożenia do modelu zagrożeń — po akceptacji                                                                                                       |

## Kontekst

Właściciel zlecił 2026-10-09 system kamer (IP, Wi-Fi, USB, kilka naraz), rozpoznawanie zapisanych użytkowników
z pamięcią długoterminową i opisywanie obrazu na polecenie. Dwa elementy tego zakresu zmieniają profil ryzyka
programu:

| Element           | Dlaczego wymaga decyzji                                                                                                 |
| ----------------- | ----------------------------------------------------------------------------------------------------------------------- |
| Wzorce twarzy     | to dane biometryczne — najwyższa kategoria wrażliwości; wyciek jest nieodwracalny (twarzy nie da się zmienić jak hasła) |
| Obraz z kamer     | pokazuje dom, dokumenty na biurku i inne osoby; wysłany do chmury wychodzi poza kontrolę użytkownika                    |
| Rozpoznanie osoby | kusi, żeby użyć go jako „logowania” — twarz da się podrobić zdjęciem albo nagraniem                                     |
| Kamery sieciowe   | tanie kamery bywają słabo zabezpieczone; strumień da się podstawić                                                      |

Projekt ma już precedens: profil głosu właściciela (`voice-speaker`) jest szyfrowany kluczem z Credential
Manager, a zweryfikowany głos **nie** zastępuje okna Brokera poza wąskim, jawnie włączanym wyjątkiem.
Sesja AI nie jest prawnikiem — poniżej są zasady techniczne, nie opinia prawna.

## Decyzja (propozycja)

1. **Kamery są domyślnie wyłączone.** Pierwsze włączenie przechodzi przez kreator ze zgodą. Każda kamera ma
   własny przełącznik, jest przełącznik główny, a kill-switch zamyka wszystkie strumienie.
2. **Działająca kamera jest zawsze widoczna** (wskaźnik w belce i pigułce głosowej); nie ma ustawienia, które
   go ukrywa.
3. **Alfa nie nagrywa.** Klatki żyją w pamięci do końca zapytania. W rozmowie zostaje miniatura klatki, do
   której odnosił się opis; znika razem z sesją.
4. **Wzorce twarzy powstają tylko za wyraźną zgodą zapisywanej osoby**, w kreatorze, przy kamerze. Alfa nie
   tworzy wzorców osób, które się nie zapisały, i nie ocenia emocji, wieku ani płci.
5. **Wzorce zostają na komputerze**: zaszyfrowane, z kluczem w Windows Credential Manager; nie wchodzą do
   eksportu `.alfa`, kopii w chmurze, logów ani diagnostyki. Usunięcie osoby kasuje je nieodwracalnie.
6. **Rozpoznanie twarzy nie jest autoryzacją.** Nie zatwierdza akcji, nie podnosi autonomii, nie odblokowuje
   sekretów i nie jest dowodem dla Brokera. Służy personalizacji: imię, pamięć rozmów, forma zwracania się.
7. **Obraz do modelu w chmurze — tylko za osobną zgodą per kamera**, domyślnie wyłączoną, nigdy w sesji
   prywatnej. Przy każdej takiej odpowiedzi widać, do kogo obraz poszedł.
8. **Obraz jest treścią niezaufaną.** Tekst w kadrze nie jest poleceniem; klatka i opis dostają ten sam znacznik
   co strona WWW czy mail.
9. **Kamery sieciowe tylko z sieci lokalnej i z listy dozwolonych adresów**; dekoder w osobnym procesie; hasła
   w Credential Manager.
10. **Agentki sięgają do kamer wyłącznie przez Brokera** (zdolność `camera.view`, narzędzia `camera.look`
    i `camera.who`); wtyczki nie dostają tej zdolności bez osobnej zgody.

## Opcje, które właściciel może wybrać inaczej

| Punkt                             | Propozycja                             | Alternatywy                    |
| --------------------------------- | -------------------------------------- | ------------------------------ |
| Obraz do chmury (7)               | za zgodą per kamera                    | nigdy · zgoda globalna         |
| Osoba nieznana                    | rozmawia jako gość bez pamięci osób    | tylko zapisani mogą rozmawiać  |
| Miniatura w rozmowie (3)          | zostaje w sesji                        | nie zapisywać żadnej klatki    |
| Rozpoznawanie w tle               | wyłączone domyślnie, włącza użytkownik | tylko na żądanie („kto to?”)   |
| Wzorce w kopii zapasowej lokalnej | nie                                    | tak, zaszyfrowane hasłem kopii |

## Konsekwencje

- Zadania C3.1–C3.7 (kamery i opis obrazu) nie dotykają biometrii i mogą ruszyć przed podpisem tego ADR;
  C3.8–C3.13 (osoby) czekają na podpis.
- Do `THREAT_MODEL.md` dochodzą scenariusze: podstawiona kamera, polecenia w obrazie, zdjęcie zamiast twarzy,
  wyciek wzorców, podgląd przez cudzą agentkę albo wtyczkę, kamera sieciowa jako punkt wejścia.
- Do `evals/` dochodzi zestaw ataków obrazem (napisy z poleceniami w kadrze) — zamrażany jak pozostałe zestawy.
- Polityki Brokera dostają nową zdolność i poziomy ryzyka dla `camera.*` — obszar Jądra, przegląd człowieka.
- Przed wydaniem publicznym: opinia prawnika o danych biometrycznych i o zakazach AI Act (pytania w
  `docs/plan/10-zgodnosc-prawna.md`).
- Modele twarzy muszą mieć licencję pozwalającą na użycie w produkcie — sprawdzenie jest warunkiem zadania C3.8.

## Czego ten ADR nie rozstrzyga

Wyboru konkretnych modeli (wykrywanie twarzy, wzorzec, model widzący), progów trafności i czasu, formatu
przechowywania wzorców — to ustalą SPEC v1 modułów i pomiary na sprzęcie.
