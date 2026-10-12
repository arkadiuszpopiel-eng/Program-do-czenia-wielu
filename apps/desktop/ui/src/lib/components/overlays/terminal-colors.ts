// Kolory emulatora terminala z tokenów motywu. Emulator rozumie tylko `#rrggbb` / `rgb()`,
// a tokeny stylów „Szkło” i „Next” to `light-dark(…)`, `oklch(…)` albo kolory półprzezroczyste —
// podane wprost dawały czarny terminal z białym tekstem także w jasnym motywie. Dlatego każdy token
// jest rozwiązywany przez przeglądarkę (element próbny + płótno 1 × 1) do nieprzezroczystego `#rrggbb`.

export interface Rgba {
  readonly r: number;
  readonly g: number;
  readonly b: number;
  /** 0–1. */
  readonly a: number;
}

const hex2 = (value: number) =>
  Math.max(0, Math.min(255, Math.round(value)))
    .toString(16)
    .padStart(2, '0');

export const toHex = (c: Pick<Rgba, 'r' | 'g' | 'b'>): string =>
  `#${hex2(c.r)}${hex2(c.g)}${hex2(c.b)}`;

/** Kolor półprzezroczysty położony na nieprzezroczystym podkładzie. */
export function over(top: Rgba, base: Pick<Rgba, 'r' | 'g' | 'b'>): Rgba {
  const mix = (t: number, b: number) => t * top.a + b * (1 - top.a);
  return { r: mix(top.r, base.r), g: mix(top.g, base.g), b: mix(top.b, base.b), a: 1 };
}

/** Jasność względna 0–1 (wystarczająco dokładna, by odróżnić jasny motyw od ciemnego). */
export const luminance = (c: Pick<Rgba, 'r' | 'g' | 'b'>): number =>
  (0.2126 * c.r + 0.7152 * c.g + 0.0722 * c.b) / 255;

/** Podkład pod kolory półprzezroczyste: ciemny, gdy tekst motywu jest jasny, i odwrotnie. */
export function baseFor(text: Pick<Rgba, 'r' | 'g' | 'b'>): Rgba {
  return luminance(text) > 0.5
    ? { r: 0x13, g: 0x14, b: 0x17, a: 1 }
    : { r: 0xfa, g: 0xfa, b: 0xfb, a: 1 };
}

/** Kolor tokenu tak, jak go widzi przeglądarka (`null` — brak płótna albo nieznany token). */
function resolve(name: string): Rgba | null {
  const probe = document.createElement('span');
  probe.style.display = 'none';
  probe.style.color = `var(${name})`;
  document.body.append(probe);
  const computed = getComputedStyle(probe).color;
  probe.remove();
  const canvas = document.createElement('canvas');
  canvas.width = 1;
  canvas.height = 1;
  const ctx = canvas.getContext('2d', { willReadFrequently: true });
  if (!ctx || !computed) return null;
  ctx.clearRect(0, 0, 1, 1);
  ctx.fillStyle = computed;
  ctx.fillRect(0, 0, 1, 1);
  const [r = 0, g = 0, b = 0, a = 255] = ctx.getImageData(0, 0, 1, 1).data;
  // Płótno trzyma kolor przemnożony przez alfę — przy bardzo małej alfie wynik jest niepewny.
  return a === 0 ? null : { r, g, b, a: a / 255 };
}

export interface TerminalColors {
  readonly background: string;
  readonly foreground: string;
  readonly cursor: string;
  readonly selectionBackground: string;
}

/** Kolory terminala dla bieżącego stylu i motywu — zawsze nieprzezroczyste `#rrggbb`. */
export function terminalColors(): TerminalColors {
  const text = resolve('--alfa-color-text') ?? { r: 0xe6, g: 0xe6, b: 0xe6, a: 1 };
  const base = baseFor(text);
  const solid = (name: string, fallback: Rgba) => toHex(over(resolve(name) ?? fallback, base));
  return {
    background: solid('--alfa-color-bg', base),
    foreground: toHex(over(text, base)),
    cursor: solid('--alfa-color-focus', { r: 0x4f, g: 0x8c, b: 0xff, a: 1 }),
    selectionBackground: solid('--alfa-color-border-strong', { r: 0x80, g: 0x86, b: 0x96, a: 1 }),
  };
}
