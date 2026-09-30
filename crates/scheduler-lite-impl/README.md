# scheduler-lite-impl

Implementacja `scheduler-lite`: `SchedulerModule` = `SchedulerLite` + `Module` (`always`, `inproc`,
RAM ≤ 1 MB, brak I/O). Rdzeń z kontraktu (`Core`) z otoczeniem `BusHost`: zegar monotoniczny tokio
(ms od startu), zadanie **timera** (śpi do najbliższego terminu — timeout żądania albo koniec
rezerwacji przekazania — i budzi się przy zmianie terminów), zadanie **publikujące** zdarzenia
`scheduler.lease.*` na magistralę (decyzje są synchroniczne, publikacja asynchroniczna).
`stop()` działa jak kill-switch dla modułu (dzierżawy odebrane, czekające anulowane).
`set_policy(resource, policy)` nadpisuje polityki (np. z `[scheduler]` w konfiguracji).

Testy: kontrakt współdzielony na zatrzymanym zegarze tokio, manifest, cykl życia, zdarzenia na
`core-bus-fake`, **100 współbieżnych scenariuszy** (5 zadań, zagnieżdżone dzierżawy, punkty atomowe):
0 nakładań, wszystko zwolnione (w przebiegu: ~5300 przyznań, ~1400 timeoutów, ~200 rozwiązanych
zakleszczeń), wywłaszczenie przez mowę użytkownika — najgorsza decyzja w 100 próbach < 5 ms
(czas rzeczywisty; w praktyce mikrosekundy).
