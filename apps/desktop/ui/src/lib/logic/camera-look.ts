// „Opisz mi to, co widzisz”: rozpoznanie prośby o spojrzenie przez kamerę w tekście użytkownika
// i wybór kamery. Gdy kamery są włączone, do takiej wiadomości dołączane jest zdjęcie z kamery —
// to prośba użytkownika wyrażona wprost, nie samodzielna decyzja agentki. Rozpoznanie jest
// ostrożne: zdjęcie to dane prywatne, więc lepiej nie zadziałać niż zadziałać przy zwykłym zdaniu
// („co widzisz w tym kodzie?”). Zdjęcie i tak nie jest wysyłane bez drugiego potwierdzenia.
import type { CameraView } from '../api/types-cameras';

/** Litery bez znaków diakrytycznych, małe, pojedyncze odstępy — do porównań. */
export function plain(text: string): string {
  return text
    .toLowerCase()
    .replace(/ł/g, 'l')
    .normalize('NFD')
    .replace(/[̀-ͯ]/g, '')
    .replace(/[^a-z0-9]+/g, ' ')
    .trim();
}

/** Słowa, które jednoznacznie mówią o kamerze (po `plain`). */
const CAMERA = /\b(kamer[a-z]*|kamerk[a-z]*|camera|cameras|webcam)\b/;

// Z kamerą w zdaniu: czasownik patrzenia albo pytanie o to, co widać / kto jest.
const WITH_CAMERA: readonly RegExp[] = [
  /\b(opisz|powiedz|pokaz)\b.*\bco\b.*\b(widzisz|widac|jest)\b/,
  /\bco\b.*\b(widzisz|widac|jest)\b/,
  /\b(spojrz|popatrz|zerknij)\b/,
  /\bkto\b.*\b(jest|stoi|siedzi)\b/,
  /\b(describe|tell me)\b.*\bwhat\b.*\bsee\b/,
  /\bwhat (do|can) you see\b/,
  /\blook (at|through)\b/,
];

// Bez słowa „kamera”: tylko cała wiadomość będąca samą prośbą (nic po niej — żadnego „w tym
// kodzie”, „na zrzucie”, „w załączniku”).
const WHOLE: readonly RegExp[] = [
  /^(alfa |beta |gama |delta )?(prosze )?opisz( mi)?( to)? co( teraz)? widzisz( prosze)?$/,
  /^(alfa |beta |gama |delta )?(prosze )?powiedz( mi)? co( teraz)? widzisz( prosze)?$/,
  /^co( teraz)? widzisz$/,
  /^(please )?describe what you see( please)?$/,
  /^what do you see$/,
];

/** Czy tekst jest prośbą o opis tego, co widzi kamera. */
export function wantsCameraLook(text: string): boolean {
  if (text.trimStart().startsWith('/')) return false;
  const normalized = plain(text);
  if (!normalized || normalized.length > 200) return false;
  if (WHOLE.some((pattern) => pattern.test(normalized))) return true;
  return CAMERA.test(normalized) && WITH_CAMERA.some((pattern) => pattern.test(normalized));
}

/**
 * Kamera do prośby: ta, której nazwa pada w tekście („…na kamerze Kuchnia”), a bez nazwy —
 * pierwsza włączona. `null`, gdy nie ma żadnej kamery do otwarcia.
 */
export function cameraFor(text: string, cameras: readonly CameraView[]): CameraView | null {
  const usable = cameras.filter((c) => c.enabled && c.state !== 'unplugged');
  const normalized = ` ${plain(text)} `;
  const named = usable
    .map((camera) => ({ camera, name: plain(camera.name) }))
    .filter(({ name }) => name.length >= 3 && normalized.includes(` ${name} `))
    .sort((a, b) => b.name.length - a.name.length)[0];
  return named?.camera ?? usable[0] ?? null;
}
