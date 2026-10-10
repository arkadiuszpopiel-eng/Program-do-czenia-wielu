// Teksty strony „Kamery” i wskaźnika kamery (plan v6, C3).
export const plCameras = {
  'cam.lead':
    'Kamery wbudowane i podłączone przez USB. Kamera otwiera się dopiero wtedy, gdy ktoś prosi o obraz, i sama zasypia. Kamery sieciowe (IP, Wi-Fi), opis obrazu i rozpoznawanie osób są w planie.',
  'cam.list': 'Twoje kamery',
  'cam.search': 'Szukaj kamer',
  'cam.loading': 'Szukam kamer…',
  'cam.none': 'Nie widzę żadnej kamery',
  'cam.noneHint':
    'Podłącz kamerę USB i wybierz „Szukaj kamer”. Jeśli kamera jest wbudowana, sprawdź w ustawieniach prywatności Windows, czy aplikacje mają do niej dostęp.',
  'cam.masterOff': 'Kamery są wyłączone — żadna się nie otworzy. Włącz je przełącznikiem powyżej.',
  'cam.preview': 'Podgląd',
  'cam.stopPreview': 'Zakończ podgląd',
  'cam.previewAlt': 'Obraz z kamery: {name}',
  'cam.rename': 'Zmień nazwę kamery {name}',
  'cam.nameLabel': 'Nazwa kamery',
  'cam.saveName': 'Zapisz nazwę',
  'cam.enable': 'Kamera {name} włączona',
  'cam.on': 'włączona',
  'cam.off': 'wyłączona',
  'cam.kind.usb': 'USB',
  'cam.kind.network': 'sieciowa',
  'cam.state.asleep': 'uśpiona',
  'cam.state.opening': 'otwieram…',
  'cam.state.live': 'działa',
  'cam.state.closing': 'zamykam…',
  'cam.state.failed': 'błąd',
  'cam.state.unplugged': 'odłączona',
  'cam.privacy': 'Prywatność',
  'cam.privacy.record':
    'Podgląd niczego nie zapisuje — klatka żyje w pamięci tylko na czas pokazania. Zdjęcie, które sam dołączysz do wiadomości, zostaje w rozmowie jak każdy załącznik.',
  'cam.privacy.sleep':
    'Bez podglądu kamera zasypia po kilkunastu sekundach; wyłączona kamera nie otworzy się nigdy.',
  'cam.privacy.indicator':
    'Gdy którakolwiek kamera działa, w górnej belce świeci czerwony znak kamery.',
  'cam.privacy.next':
    'Przyciskiem z aparatem w polu pisania dołączysz zdjęcie z kamery i zapytasz „opisz mi to, co widzisz” — potrzebny jest model, który przyjmuje obrazy. Agentki same nie sięgają do kamer; to następny krok planu.',
  'cam.snap': 'Dołącz zdjęcie z kamery: {name}',
  'cam.snapPick': 'Dołącz zdjęcie z kamery',
  'cam.autoLook':
    'Dołączyłam zdjęcie z kamery „{name}”. Sprawdź je i naciśnij Enter, aby wysłać — albo usuń załącznik.',
  'cam.indicator': {
    one: 'Działa {n} kamera: {names}. Otwórz ustawienia kamer',
    few: 'Działają {n} kamery: {names}. Otwórz ustawienia kamer',
    many: 'Działa {n} kamer: {names}. Otwórz ustawienia kamer',
    other: 'Działa {n} kamery: {names}. Otwórz ustawienia kamer',
  },
} as const;
