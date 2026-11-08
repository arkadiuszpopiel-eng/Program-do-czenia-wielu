// Skróty klawiszowe: parsowanie, dopasowanie zdarzeń, wykrywanie konfliktów (PLAN §14.8, §8.6).
// Reguła AltGr: na polskiej klawiaturze Ctrl+Alt = AltGr, więc Ctrl+Alt(+Shift) + a c e l n o s x z
// wpisuje ą ć ę ł ń ó ś ź ż — taki skrót jest zakazany (ADR 0010).

export interface Chord {
  readonly ctrl: boolean;
  readonly alt: boolean;
  readonly shift: boolean;
  readonly meta: boolean;
  readonly key: string;
}

export interface KeyLike {
  readonly key: string;
  readonly code: string;
  readonly ctrlKey: boolean;
  readonly altKey: boolean;
  readonly shiftKey: boolean;
  readonly metaKey: boolean;
}

export const ALTGR_LETTERS = new Set(['A', 'C', 'E', 'L', 'N', 'O', 'S', 'X', 'Z']);

/** Skróty zarezerwowane: nie można ich przypisać akcjom UI. */
export const RESERVED: Readonly<Record<string, 'reload' | 'kill_switch'>> = {
  F5: 'reload',
  'Ctrl+R': 'reload',
  'Ctrl+Shift+R': 'reload',
  'Ctrl+Shift+F12': 'kill_switch',
};

const KEY_ALIASES: Readonly<Record<string, string>> = {
  ' ': 'Space',
  Spacebar: 'Space',
  Escape: 'Esc',
  ArrowUp: 'Up',
  ArrowDown: 'Down',
  ArrowLeft: 'Left',
  ArrowRight: 'Right',
  Plus: '=',
};

const CODE_KEYS: Readonly<Record<string, string>> = {
  Slash: '/',
  Backslash: '\\',
  IntlBackslash: '\\',
  Comma: ',',
  Period: '.',
  Equal: '=',
  Minus: '-',
  NumpadAdd: '=',
  NumpadSubtract: '-',
  Numpad0: '0',
  Semicolon: ';',
  Quote: "'",
  Backquote: '`',
  BracketLeft: '[',
  BracketRight: ']',
  Space: 'Space',
};

const MODIFIER_KEYS = new Set(['Control', 'Alt', 'Shift', 'Meta', 'AltGraph', 'OS']);

function normalizeKey(key: string): string {
  const aliased = KEY_ALIASES[key] ?? key;
  return aliased.length === 1 ? aliased.toUpperCase() : aliased;
}

export function formatChord(chord: Chord): string {
  const parts: string[] = [];
  if (chord.ctrl) parts.push('Ctrl');
  if (chord.alt) parts.push('Alt');
  if (chord.shift) parts.push('Shift');
  if (chord.meta) parts.push('Win');
  parts.push(chord.key);
  return parts.join('+');
}

/** „Ctrl+Shift+F" → Chord; `null`, gdy zapis jest niepoprawny. */
export function parseChord(text: string): Chord | null {
  const raw = text.trim();
  if (!raw) return null;
  const endsWithPlus = raw.endsWith('++');
  const parts = (endsWithPlus ? raw.slice(0, -2) : raw).split('+').filter(Boolean);
  const key = endsWithPlus ? '=' : parts.pop();
  if (!key) return null;
  const chord = { ctrl: false, alt: false, shift: false, meta: false, key: normalizeKey(key) };
  for (const part of parts) {
    const p = part.toLowerCase();
    if (p === 'ctrl' || p === 'control') chord.ctrl = true;
    else if (p === 'alt') chord.alt = true;
    else if (p === 'shift') chord.shift = true;
    else if (p === 'win' || p === 'meta' || p === 'cmd') chord.meta = true;
    else return null;
  }
  return chord;
}

export function normalizeChord(text: string): string | null {
  const chord = parseChord(text);
  return chord ? formatChord(chord) : null;
}

/** Zdarzenie klawiatury → kanoniczny zapis skrótu (`null` dla samych modyfikatorów). */
export function chordFromEvent(event: KeyLike): string | null {
  if (MODIFIER_KEYS.has(event.key)) return null;
  let key: string;
  const letter = /^Key([A-Z])$/.exec(event.code);
  const digit = /^(?:Digit|Numpad)([0-9])$/.exec(event.code);
  if (letter?.[1]) key = letter[1];
  else if (digit?.[1] && (event.ctrlKey || event.altKey || event.metaKey)) key = digit[1];
  else key = CODE_KEYS[event.code] ?? normalizeKey(event.key);
  return formatChord({
    ctrl: event.ctrlKey,
    alt: event.altKey,
    shift: event.shiftKey,
    meta: event.metaKey,
    key,
  });
}

export function isAltGrConflict(text: string): boolean {
  const chord = parseChord(text);
  return Boolean(chord && chord.ctrl && chord.alt && ALTGR_LETTERS.has(chord.key));
}

export type ShortcutScope = 'app' | 'global';

export interface ShortcutDef {
  readonly id: string;
  readonly defaults: readonly string[];
  /** `global` — skrót systemowy obsługiwany przez rdzeń (RegisterHotKey), nie przez WebView. */
  readonly scope: ShortcutScope;
  readonly group: 'sessions' | 'view' | 'panels' | 'conversation' | 'voice' | 'system';
  readonly customizable: boolean;
  /** Działa także, gdy fokus jest w polu tekstowym. */
  readonly inInput: boolean;
}

export type ConflictReason = 'duplicate' | 'altgr' | 'reserved';

export interface ShortcutConflict {
  readonly chord: string;
  readonly ids: readonly string[];
  readonly reason: ConflictReason;
}

/** Skuteczne przypisania po uwzględnieniu nadpisań użytkownika (pusty napis = wyłączony). */
export function effectiveBindings(
  defs: readonly ShortcutDef[],
  overrides: Readonly<Record<string, string>>,
): Record<string, string[]> {
  const out: Record<string, string[]> = {};
  for (const def of defs) {
    const override = overrides[def.id];
    const list = override === undefined ? def.defaults : override ? [override] : [];
    out[def.id] = list.map((c) => normalizeChord(c)).filter((c): c is string => c !== null);
  }
  return out;
}

export function findConflicts(
  defs: readonly ShortcutDef[],
  overrides: Readonly<Record<string, string>> = {},
): ShortcutConflict[] {
  const bindings = effectiveBindings(defs, overrides);
  const byChord = new Map<string, string[]>();
  const conflicts: ShortcutConflict[] = [];
  for (const def of defs) {
    for (const chord of bindings[def.id] ?? []) {
      const reserved = RESERVED[chord];
      if (reserved && !(reserved === 'kill_switch' && def.id === 'system.killSwitch')) {
        conflicts.push({ chord, ids: [def.id], reason: 'reserved' });
      }
      if (isAltGrConflict(chord)) conflicts.push({ chord, ids: [def.id], reason: 'altgr' });
      const list = byChord.get(chord) ?? [];
      list.push(def.id);
      byChord.set(chord, list);
    }
  }
  for (const [chord, ids] of byChord) {
    if (ids.length > 1) conflicts.push({ chord, ids, reason: 'duplicate' });
  }
  return conflicts;
}

/** Mapa skrót → akcja dla obsługi zdarzeń (tylko zakres `app`). */
export function buildKeymap(
  defs: readonly ShortcutDef[],
  overrides: Readonly<Record<string, string>>,
): Map<string, ShortcutDef> {
  const bindings = effectiveBindings(defs, overrides);
  const map = new Map<string, ShortcutDef>();
  for (const def of defs) {
    if (def.scope !== 'app') continue;
    for (const chord of bindings[def.id] ?? []) if (!map.has(chord)) map.set(chord, def);
  }
  return map;
}

/** Skróty działające w polu tekstowym muszą mieć modyfikator albo być klawiszem funkcyjnym. */
export function allowedInInput(chord: string, def: ShortcutDef): boolean {
  if (def.inInput) return true;
  const parsed = parseChord(chord);
  if (!parsed) return false;
  return parsed.ctrl || parsed.alt || parsed.meta || /^F\d{1,2}$/.test(parsed.key);
}
