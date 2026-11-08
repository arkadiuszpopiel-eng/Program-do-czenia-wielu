# lib-sidecar-guard

Rejestr procesów pobocznych Alfy. Każdy moduł, który uruchamia silnik jako osobny proces (`llama-server`,
`whisper-server`, silniki głosu, Smart Turn), zgłasza jego numer i nazwę pliku. Powłoka:

- przy wyjściu z programu woła `kill_all()` — `kill_on_drop` nie wystarcza, bo wyjście przez `exit` nie
  niszczy obiektów procesów i silnik zostawał jako osierocony proces trzymający pamięć karty graficznej
  (znalezione testem na instancji próbnej, 2026-10-12);
- przy starcie woła `init(plik)` — rejestr z poprzedniego uruchomienia jest na dysku, więc procesy, które
  przeżyły awarię programu, są kończone (tylko gdy pod tym numerem nadal działa plik o zapisanej nazwie).

Bez `windows-rs`: sprawdzenie i zakończenie procesu robią systemowe `tasklist` i `taskkill`. Poza Windows
funkcje nic nie robią.
