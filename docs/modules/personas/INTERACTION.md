# Jarvis — sposób współpracy agentki

Zlecenie właściciela z 8.11.2026, karta [#22](https://github.com/arkadiuszpopiel-eng/Program-do-czenia-wielu/issues/22).
Robocza interpretacja do czasu odpowiedzi właściciela: rozmowa głosowa, wykonywanie zleconych zadań
i krótkie meldunki. Dotyczy Alfy, Bety, Gamy, Delty i nowych person korzystających z ustawienia wspólnego.

## Kontrakt zachowania

- `standard` zachowuje obecny sposób odpowiedzi; `jarvis` dodaje profil współpracy do instrukcji modelu.
- `agents.interaction_mode` jest wspólnym ustawieniem domyślnym (domyślnie `standard`).
- `agents.<id>.interaction_mode` dla person wbudowanych: `inherit` / `standard` / `jarvis`.
- Jarvis zachowuje imię, głos, rodzaj żeński, role i pamięć persony. To nazwa sposobu współpracy.
- Profil przekazuje imię i jego narzędnik z bieżącej persony, także własnej. Przy przedstawianiu się
  model korzysta z tych form. Stosuje długość wskazaną przez właściciela; cechy i role opisuje na prośbę.
- Agentka podaje krótki zamiar, wykonuje zlecone czynności dostępnymi narzędziami, a następnie meldunek
  z wynikiem sprawdzenia. Przy dłuższym zadaniu przekazuje krótkie informacje o postępie.
- Głos korzysta z istniejących fraz wywoławczych i weryfikacji mówcy. Zmiana profilu nie otwiera mikrofonu.
- Mikrofon włącza właściciel dotychczasową kontrolką. Brak silników pozostaje jawnym stanem niedostępności.
- Nie zmienia poziomu L0–L4, Max dostępu, roli, kłódki, narzędzi, kosztów, czasu ani automatycznych przedłużeń.
- W trybie Plan agentka przygotowuje plan; w innych trybach działa w zakresie decyzji Brokera.
- Zmiana obowiązuje od następnej odpowiedzi / nowego podzadania. Wznowienie checkpointu zachowuje instrukcje
  przypięte do tego przebiegu; wewnętrzny format werdyktu Krytyczki pozostaje obowiązujący.

## Integracja

Instrukcje profilu trafiają do zwykłej rozmowy (w tym transkrypcji głosu), bezpośredniego przebiegu agentki,
podzadania uruchamianego przez Dyrygentkę i zadania w tle. Ustawienie jest zapisywane przez istniejący
magazyn konfiguracji i walidowane schematem. Panel Agentki pozwala wybrać profil osobno dla czterech person wbudowanych.

Kreator przy bibliotece własnych agentek udostępnia ten sam wspólny profil Standardowy/Jarvis.
Opis wskazuje wspólny zakres: własne persony dziedziczą go, podobnie wbudowane z wyborem
„Jak ustawienie wspólne”. Osobny Standardowy/Jarvis wbudowanej persony ma nadal pierwszeństwo.
Zapis zmienia profil od następnej odpowiedzi; nie uruchamia mikrofonu ani zadania.

## Sprawdzenie

Testy mają potwierdzać propagację ustawienia i brak zmiany uprawnień. Odpowiedź modelu na sam prompt
nie jest dowodem wykonania zadania. Testy na atrapach i w przeglądarce są oddzielone od odsłuchu i próby
na rzeczywistym modelu oraz mikrofonie.

Własna Luna z publicznego Kreatora ma oddzielną ręczną próbę lokalnego Bielika:5 rozmów Standard
i5 Jarvis po restarcie katalogu i obsad (Standard jest kontrolą przed restartem).
Wszystkie10 rozmów oraz role są przygotowane przed restartem;
test nie przypisuje ich ponownie. Rola rozmowy nie ma narzędzi; raport zachowuje instrukcje
routera i faktycznego dostawcy, autora oraz obcięcie. Nowa próba nazywa się
`custom_persona_on_local_model_after_catalog_and_cast_restart` w `jarvis_live.rs`.
[Wyniki i ograniczenia modelu](../../reviews/2026-11-09-jarvis-model-repair.md);
[wcześniejszy raport #43](../../reviews/2026-11-09-live-custom.md) pozostaje historyczny.

Przegląd w świeżej sesji przed scaleniem; SPEC nie zamyka żadnego kryterium sprzętowego.

W bibliotece Kreatora przy wspólnym Jarvisie przycisk **Sterowanie głosem** zamyka Ustawienia
i otwiera panel **Głos**. To nawigacja: zachowuje profil i stan mikrofonu; nie uruchamia rozmowy
ani nasłuchu. Osobne wybory wbudowanych person nadal mają pierwszeństwo.
