# WebView2: anulowanie kończy się asynchronicznie, ponowienie wymaga done

Windows120 na a2b47cf2: **62/1/5 pominiętych**, porażka tasks_retry w z-lifecycle.
Surowy log i metadane artefaktu zachowane. Duży ZIP49339722B pozostał naGitHub:
artefakt13922671592, SHA25623a720d7fdda7475c6e215b446a85ab4a8cba75353dd5127b34e596f5e63ec86;
nie deklarujemy obejrzenia zrzutów lub lokalnego rozpakowania tegoZIP.
Poprawiony test odczytuje TaskInfo przez prawdziweIPC i czeka na done przed jednym
retry. Istniejący expect15s/test120s/retries0 i oczekiwanie nowegoID bez zmian.
Lokalnie: kontraktCore tasks **3/0/0** (porty/model testowe), pnpm lint/format0,
Playwright tylko **listowanie68**, bez zaliczenia68prób. NowyWindowsGREEN wymagany.
Manifest wiąże źródła, surowe błędy i komendy. Brak zmian produkcji/polityk/progów.
