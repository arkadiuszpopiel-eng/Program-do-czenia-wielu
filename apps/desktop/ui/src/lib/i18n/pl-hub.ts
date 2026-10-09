// Kreator „Dodaj dostawcę”, wynik testu klucza i stany kont (pl — źródło kluczy).
export const plHub = {
  'hub.state.unconfigured': 'nieskonfigurowane',
  'hub.state.testing': 'testuję…',
  'hub.state.ok': 'działa',
  'hub.state.invalid': 'klucz odrzucony',
  'hub.state.rate_limited': 'limit zapytań',
  'hub.state.disabled': 'wyłączone',
  'hub.test': 'Testuj połączenie',
  'hub.toModels': 'Modele i kolejki',
  'hub.rotate': 'Zmień klucz',
  'hub.rotateTitle': 'Nowy klucz dla konta „{label}”',
  'hub.rotateSubmit': 'Sprawdź i zamień klucz',
  'hub.rotated': 'Nowy klucz został zapisany w miejsce poprzedniego.',
  'hub.rotateKept': 'Nowy klucz nie został zapisany — konto dalej używa poprzedniego.',
  'hub.rotateOther': 'Wklej inny klucz',
  'hub.disable': 'Wyłącz',
  'hub.enable': 'Włącz',
  'hub.why': 'Co zrobić',
  'hub.reason.ok': 'działa',
  'hub.reason.restricted': 'działa · ograniczone uprawnienia',
  'hub.reason.rate_limited': 'limit zapytań',
  'hub.reason.invalid_key': 'klucz odrzucony',
  'hub.reason.no_credit': 'brak środków na koncie',
  'hub.reason.region': 'kraj nieobsługiwany',
  'hub.reason.refused': 'dostęp odmówiony',
  'hub.reason.timeout': 'brak odpowiedzi',
  'hub.reason.network': 'błąd połączenia',
  'hub.reason.unsupported': 'bez testu',

  'wiz.title': 'Dodaj dostawcę',
  'wiz.stepOf': 'Krok {n} z {total}: {name}',
  'wiz.step.provider': 'Dostawca',
  'wiz.step.key': 'Klucz',
  'wiz.step.test': 'Test',
  'wiz.step.models': 'Modele',
  'wiz.step.assign': 'Przypisanie',
  'wiz.step.limit': 'Limit',
  'wiz.filter': 'Filtruj dostawców',
  'wiz.label': 'Nazwa konta',
  'wiz.key': 'Klucz API',
  'wiz.keyHint':
    'Wklej klucz. Nie zapisujemy go w plikach — trafia do Menedżera poświadczeń Windows.',
  'wiz.baseUrl': 'Adres endpointu (base URL)',
  'wiz.baseUrlHint':
    'Adres API tego dostawcy z jego dokumentacji, zaczyna się od https://. Klucz zostanie wysłany tylko pod ten adres.',
  'wiz.keyFail': 'Nie zapisano klucza: {error}',
  'wiz.testing': 'Sprawdzam klucz u dostawcy {name}…',
  'wiz.keyTitle': 'Klucz API od {name}',
  'wiz.howTitle': 'Skąd wziąć klucz',
  'wiz.how.open': 'Otwórz w przeglądarce stronę kluczy i zaloguj się:',
  'wiz.how.copy': 'Kopiuj adres',
  'wiz.how.copied': 'Skopiowano',
  'wiz.how.create':
    'Utwórz nowy klucz (przycisk „Create new secret key” albo „Create key”) i skopiuj go od razu — później nie da się go już podejrzeć.',
  'wiz.how.paste': 'Wklej klucz poniżej (Ctrl+V).',
  'wiz.how.billing':
    'API to osobna, płatna usługa {name}: abonament czatu (np. ChatGPT Plus, Claude Pro) jej nie obejmuje. Klucz zadziała dopiero, gdy konto API ma doładowane środki albo podpiętą kartę.',
  'wiz.showKey': 'Pokaż klucz',
  'wiz.labelHint': 'Tylko dla Ciebie — tak konto będzie nazwane na liście.',
  'wiz.checkKey': 'Sprawdź klucz',
  'wiz.check.ok': 'Wygląda jak klucz. Czy działa, sprawdzę u dostawcy.',
  'wiz.check.notKey':
    'To nie wygląda na klucz API — zostały spacje albo polskie znaki. Wklej sam klucz, bez opisu.',
  'wiz.check.cliToken':
    'To token logowania narzędzia Claude Code, a nie klucz API. Utwórz klucz API na stronie podanej wyżej.',
  'wiz.check.other':
    'To wygląda na klucz {other}, a wybrany dostawca to {name}. Jeśli to pomyłka — wróć do kroku „Dostawca”.',
  'wiz.check.prefix':
    'Klucze {name} zwykle zaczynają się od „{prefix}”. Sprawdź, czy skopiowano cały klucz.',
  'wiz.check.short':
    'To bardzo krótki tekst — klucze API są dłuższe. Sprawdź, czy skopiowano całość.',
  'wiz.check.tidied':
    'Usunę dodatki ze schowka (cudzysłowy, „NAZWA=”, spacje, znaki nowej linii) — do zapisu pójdzie sam klucz.',
  'wiz.testAgain': 'Sprawdź ponownie',
  'wiz.fixTitle': 'Co zrobić',
  'wiz.detailTitle': 'Co dokładnie odpowiedział {name}',
  'wiz.detail.status': 'Kod HTTP',
  'wiz.detail.code': 'Kod błędu',
  'wiz.detail.message': 'Komunikat',
  'wiz.verdict.ok': 'Klucz działa',
  'wiz.verdict.restricted': 'Klucz działa, ale ma ograniczone uprawnienia',
  'wiz.verdict.rate_limited': 'Dostawca chwilowo ogranicza zapytania',
  'wiz.verdict.invalid_key': '{name} nie przyjął tego klucza',
  'wiz.verdict.no_credit': 'Klucz jest poprawny, ale konto nie ma środków',
  'wiz.verdict.region': '{name} nie obsługuje tego kraju lub regionu',
  'wiz.verdict.refused': '{name} odmówił dostępu',
  'wiz.verdict.timeout': '{name} nie odpowiedział na czas',
  'wiz.verdict.network': 'Nie udało się połączyć z {name}',
  'wiz.verdict.unsupported': 'Tego klucza nie da się sprawdzić z góry',
  'wiz.why.restricted':
    'Klucz jest ważny, ale nie ma prawa do pobrania listy modeli. Rozmowa będzie działać, gdy podasz nazwę modelu, z którego Alfa ma korzystać.',
  'wiz.why.rate_limited':
    'Limit zapytań został na chwilę wyczerpany, więc dostawca nie sprawdził jeszcze klucza. To nie znaczy, że klucz jest zły.',
  'wiz.why.invalid_key':
    'Dostawca odpowiedział, że taki klucz nie istnieje, wygasł albo został usunięty. Najczęściej to niepełna kopia albo klucz z innego konta.',
  'wiz.why.no_credit':
    'Dostawca rozpoznał klucz, ale odmawia, bo konto API nie ma środków albo aktywnych płatności.',
  'wiz.why.region':
    'Dostawca blokuje dostęp do API z tego kraju lub sieci. Klucz może być poprawny.',
  'wiz.why.refused':
    'Dostawca odpowiedział odmową z powodu innego niż sam klucz: wyłączone API na koncie, brak dostępu do projektu albo zapora. Klucz może być poprawny.',
  'wiz.why.timeout': 'Dostawca nie odpowiedział w wyznaczonym czasie.',
  'wiz.why.network':
    'Połączenie z serwerem dostawcy nie powiodło się. To nie musi być wina klucza.',
  'wiz.why.unsupported':
    'Ten dostawca albo adres nie daje sposobu na sprawdzenie klucza przed rozmową. Klucz jest zapisany — czy działa, okaże się przy pierwszej rozmowie.',
  'wiz.fix.restricted.1':
    'Podaj nazwę modelu (np. gpt-4o-mini) — dokładnie tak, jak podaje ją dostawca.',
  'wiz.fix.restricted.2':
    'Albo nadaj kluczowi prawo odczytu modeli („Models: Read”) lub utwórz klucz z pełnymi uprawnieniami („All”): {url}. Potem sprawdź ponownie.',
  'wiz.fix.rate_limited.1':
    'Odczekaj minutę i sprawdź ponownie. Konto możesz zostawić — jeśli klucz jest dobry, zacznie działać samo.',
  'wiz.fix.invalid_key.1':
    'Utwórz nowy klucz ({url}) i skopiuj go przyciskiem kopiowania — w całości.',
  'wiz.fix.invalid_key.2':
    'Sprawdź, czy klucz pochodzi z konta i projektu, z którego chcesz korzystać, i czy nie został usunięty.',
  'wiz.fix.invalid_key.3': 'Upewnij się, że to klucz {name}, a nie innego dostawcy.',
  'wiz.fix.invalid_key.4': 'Wklej nowy klucz — przycisk poniżej.',
  'wiz.fix.no_credit.1':
    'Doładuj konto API albo dodaj kartę w rozliczeniach dostawcy (to osobne od abonamentu czatu).',
  'wiz.fix.no_credit.2':
    'Po doładowaniu sprawdź ponownie — nowego klucza nie trzeba. Zmiana bywa widoczna po kilku minutach.',
  'wiz.fix.region.1':
    'Sprawdź listę obsługiwanych krajów u dostawcy; VPN albo sieć firmowa mogą zmieniać widoczny kraj.',
  'wiz.fix.refused.1':
    'Rozwiń „Co dokładnie odpowiedział…” poniżej — dostawca zwykle podaje tam powód.',
  'wiz.fix.refused.2':
    'Sprawdź w ustawieniach konta u dostawcy ({url}), czy API jest włączone, a klucz należy do właściwego projektu; potem sprawdź ponownie.',
  'wiz.fix.timeout.1': 'Sprawdź połączenie z Internetem i sprawdź ponownie.',
  'wiz.fix.timeout.2': 'Jeśli się powtarza — strona stanu dostawcy powie, czy ma awarię.',
  'wiz.fix.network.1': 'Sprawdź połączenie z Internetem, zaporę i VPN, potem sprawdź ponownie.',
  'wiz.fix.network.2': 'Przy własnym adresie endpointu sprawdź, czy jest wpisany poprawnie.',
  'wiz.fix.unsupported.1':
    'Jeśli nie ma listy modeli, podaj nazwę modelu, z którego Alfa ma korzystać — dokładnie tak, jak podaje ją dostawca.',
  'wiz.testOk': 'Połączono · {latency}',
  'wiz.fixKey': 'Popraw klucz',
  'wiz.keepAccount': 'Zostaw konto i zamknij',
  'wiz.finishLater': 'Zamknij — resztę ustawię później',
  'wiz.providerSite': 'strona dostawcy',
  'wiz.modelName': 'Nazwa modelu',
  'wiz.modelSave': 'Zapisz model',
  'wiz.modelSet': 'Alfa będzie używać modelu „{model}”.',
  'wiz.modelsFound': {
    one: 'Wykryto {n} model',
    few: 'Wykryto {n} modele',
    many: 'Wykryto {n} modeli',
    other: 'Wykryto {n} modelu',
  },
  'wiz.tasks': 'Klasy zadań',
  'wiz.task.chat': 'Rozmowa',
  'wiz.task.code': 'Kod',
  'wiz.task.planning': 'Planowanie',
  'wiz.task.summarize': 'Streszczanie',
  'wiz.agents': 'Agentki',
  'wiz.voice': 'Głos',
  'wiz.stt': 'Rozpoznawanie mowy (STT)',
  'wiz.tts': 'Synteza mowy (TTS)',
  'wiz.limitToggle': 'Limit kosztów dla tego dostawcy',
  'wiz.limitAmount': 'Kwota miesięcznie (zł)',
  'wiz.finish': 'Zakończ',
  'wiz.skip': 'Pomiń — dodam później',
  'wiz.added': 'Dodano konto „{label}".',
} as const;
