# watchdog-fake

Atrapa watchdoga: rejestruje heartbeaty, awarie (`Health::Failing`, `report_crash`) i wywołania
kill-switcha jako zdarzenia (nic nie jest zabijane), safe-mode wymuszany z testu (`force_safe_mode`),
akcje `tick` ze skryptu. Przechodzi współdzielony test kontraktowy.
