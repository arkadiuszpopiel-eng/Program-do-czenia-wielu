// Odpowiedzi atrapy z artefaktami dla kanwy (C2.8): strona HTML (ze skryptem, który nie może
// zadziałać), rysunek SVG, notatka Markdown oraz długi dokument z nagłówkami.
const FENCE = '```';

export const CANVAS_ARTIFACTS_REPLY = [
  'Przygotowałam stronę, rysunek i notatkę. Każdy artefakt otworzysz w kanwie obok rozmowy.',
  '',
  `${FENCE}html`,
  '<!doctype html>',
  '<html>',
  '<head><title>Karta projektu</title><style>h1{color:#3730a3}</style></head>',
  '<body>',
  '<h1>Karta projektu</h1>',
  '<p>Termin oddania: 30 października.</p>',
  '<script>window.top.__TAURI_INTERNALS__.invoke("app_quit")</script>',
  '</body>',
  '</html>',
  FENCE,
  '',
  `${FENCE}svg`,
  '<svg viewBox="0 0 120 60" xmlns="http://www.w3.org/2000/svg">',
  '<rect x="4" y="4" width="112" height="52" rx="8" fill="#0e7490"/>',
  '<text x="60" y="36" text-anchor="middle" fill="#ffffff">Alfa</text>',
  '</svg>',
  FENCE,
  '',
  `${FENCE}markdown`,
  '# Notatka ze spotkania',
  '',
  '- budżet zatwierdzony',
  '- kolejne spotkanie w piątek',
  FENCE,
].join('\n');

const PARAGRAPH =
  'Sprzedaż w trzecim kwartale wzrosła o dwanaście procent rok do roku, a koszty stałe pozostały ' +
  'na poziomie z poprzedniego kwartału. Największy wkład miał segment B2B, w którym nowi klienci ' +
  'z sektora publicznego podpisali umowy roczne.';

export const CANVAS_DOCUMENT_REPLY = [
  '# Raport kwartalny',
  '',
  PARAGRAPH,
  '',
  '## Przychody',
  '',
  PARAGRAPH,
  '',
  PARAGRAPH,
  '',
  '## Ryzyka',
  '',
  '- dłuższy cykl płatności w sektorze publicznym',
  '- kurs EUR wobec kontraktów w złotych',
  '',
  PARAGRAPH,
].join('\n');
