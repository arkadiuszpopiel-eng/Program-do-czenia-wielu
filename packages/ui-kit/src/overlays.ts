// Menu kontekstowe i podpowiedzi rysowane przez Alfę zamiast przez system (zgłoszenie właściciela,
// 2026-10-09: po kliknięciu prawym przyciskiem pojawiało się menu WebView2 z „Emoji”, „Kierunek
// pisania”, „Wykonaj inspekcję”; atrybut `title` dawał systemowy dymek). Bez frameworka — te same
// funkcje działają w oknie głównym, Szybkim pytaniu i pigułce. Wygląd: `base.css` (`.alfa-ctx`,
// `.alfa-tip`). Elementy trafiają do warstwy wierzchniej (`popover`), więc są też nad oknami dialogowymi.
import { place, show } from './popover';
import { uiZoom } from './zoom';

/** Teksty menu kontekstowego (tłumaczy aplikacja). */
export interface ContextMenuLabels {
  readonly menu: string;
  readonly undo: string;
  readonly cut: string;
  readonly copy: string;
  readonly paste: string;
  readonly selectAll: string;
  /** Komunikat, gdy schowka nie da się odczytać (np. pusta zgoda WebView). */
  readonly pasteFailed: string;
}

/** Teksty menu w obu językach interfejsu — wspólne dla wszystkich okien. */
export const CONTEXT_MENU_LABELS: Readonly<Record<'pl' | 'en', ContextMenuLabels>> = {
  pl: {
    menu: 'Menu edycji',
    undo: 'Cofnij',
    cut: 'Wytnij',
    copy: 'Kopiuj',
    paste: 'Wklej',
    selectAll: 'Zaznacz wszystko',
    pasteFailed: 'Nie udało się odczytać schowka — użyj Ctrl+V.',
  },
  en: {
    menu: 'Edit menu',
    undo: 'Undo',
    cut: 'Cut',
    copy: 'Copy',
    paste: 'Paste',
    selectAll: 'Select all',
    pasteFailed: 'Could not read the clipboard — use Ctrl+V.',
  },
};

export interface ContextMenuOptions {
  readonly labels: () => ContextMenuLabels;
  /**
   * Odczyt tekstu schowka. WebView2 odmawia stronie `navigator.clipboard.readText()`, więc aplikacja
   * podaje tu polecenie rdzenia; bez tego używany jest schowek przeglądarki.
   */
  readonly readClipboard?: () => Promise<string | null>;
  /** Pokazanie komunikatu (toast); bez tego błąd wklejania trafia tylko do konsoli. */
  readonly notify?: (message: string) => void;
}

type Editable = HTMLInputElement | HTMLTextAreaElement;
type Action = 'undo' | 'cut' | 'copy' | 'paste' | 'selectAll';

const TEXT_INPUTS = new Set(['text', 'search', 'url', 'email', 'tel', 'password', 'number', '']);
const SHORTCUTS: Readonly<Record<Action, string>> = {
  undo: 'Ctrl+Z',
  cut: 'Ctrl+X',
  copy: 'Ctrl+C',
  paste: 'Ctrl+V',
  selectAll: 'Ctrl+A',
};

function editableOf(target: EventTarget | null): Editable | null {
  if (target instanceof HTMLTextAreaElement) return target;
  if (target instanceof HTMLInputElement && TEXT_INPUTS.has(target.type)) return target;
  return null;
}

/**
 * Menu kontekstowe Alfy: w polach tekstowych — cofnij, wytnij, kopiuj, wklej, zaznacz wszystko;
 * przy zaznaczonym tekście poza polem — kopiuj. W pozostałych miejscach prawy przycisk nie robi
 * nic (menu systemowe jest zawsze wyłączone). Zwraca funkcję sprzątającą.
 */
export function installContextMenu(options: ContextMenuOptions): () => void {
  let menu: HTMLElement | null = null;
  let active = -1;
  let buttons: HTMLButtonElement[] = [];

  const close = (): void => {
    menu?.remove();
    menu = null;
    buttons = [];
    active = -1;
  };

  const highlight = (index: number): void => {
    active = index;
    buttons.forEach((b, at) => b.classList.toggle('active', at === index));
    menu?.setAttribute('aria-activedescendant', buttons[index]?.id ?? '');
  };

  const run = async (action: Action, field: Editable | null): Promise<void> => {
    close();
    field?.focus();
    if (action === 'selectAll') {
      if (field) field.select();
      else document.execCommand('selectAll');
      return;
    }
    if (action !== 'paste') {
      document.execCommand(action);
      return;
    }
    try {
      const read = options.readClipboard ?? (() => navigator.clipboard.readText());
      const text = await read();
      // Pole mogło stracić fokus w czasie odczytu (np. okno potwierdzenia przeglądarki).
      field?.focus();
      // `insertText` zachowuje historię cofania i wysyła zdarzenie `input` (pola powiązane).
      if (text) document.execCommand('insertText', false, text);
    } catch {
      const message = options.labels().pasteFailed;
      if (options.notify) options.notify(message);
      else console.warn(`Alfa: ${message}`);
    }
  };

  const open = (event: MouseEvent, field: Editable | null, selected: boolean): void => {
    close();
    const labels = options.labels();
    const secret = field?.type === 'password';
    const editable = Boolean(field && !field.readOnly && !field.disabled);
    const marked = field ? (field.selectionEnd ?? 0) > (field.selectionStart ?? 0) : selected;
    const items: readonly (readonly [Action, string, boolean] | null)[] = field
      ? [
          ['undo', labels.undo, editable],
          null,
          ['cut', labels.cut, editable && marked && !secret],
          ['copy', labels.copy, marked && !secret],
          ['paste', labels.paste, editable],
          null,
          ['selectAll', labels.selectAll, field.value.length > 0],
        ]
      : [['copy', labels.copy, true]];

    const el = document.createElement('div');
    el.className = 'alfa-ctx';
    el.setAttribute('role', 'menu');
    el.setAttribute('aria-label', labels.menu);
    el.setAttribute('popover', 'manual');
    el.tabIndex = -1;
    for (const item of items) {
      if (!item) {
        const sep = document.createElement('div');
        sep.className = 'sep';
        sep.setAttribute('role', 'separator');
        el.append(sep);
        continue;
      }
      const [action, label, enabled] = item;
      const button = document.createElement('button');
      button.type = 'button';
      button.id = `alfa-ctx-${action}`;
      button.setAttribute('role', 'menuitem');
      button.disabled = !enabled;
      const name = document.createElement('span');
      name.textContent = label;
      const keys = document.createElement('span');
      keys.className = 'keys';
      keys.textContent = SHORTCUTS[action];
      button.append(name, keys);
      // Bez przejęcia fokusu: zaznaczenie w polu musi przetrwać do wykonania akcji.
      button.addEventListener('pointerdown', (e) => e.preventDefault());
      button.addEventListener('click', () => void run(action, field));
      button.addEventListener('pointerenter', () => highlight(buttons.indexOf(button)));
      el.append(button);
      if (enabled) buttons.push(button);
    }
    document.body.append(el);
    show(el);
    const zoom = uiZoom();
    place(el, event.clientX / zoom, event.clientY / zoom);
    menu = el;
  };

  const onContextMenu = (event: MouseEvent): void => {
    // Menu systemowe nie pojawia się nigdy; nasze — tylko tam, gdzie ma co zaoferować.
    event.preventDefault();
    if (event.target instanceof Element && event.target.closest('.alfa-ctx')) return;
    const field = editableOf(event.target);
    const selected = (document.getSelection()?.toString() ?? '') !== '';
    if (field) {
      field.focus();
      open(event, field, selected);
    } else if (selected) open(event, null, true);
    else close();
  };

  const onKey = (event: KeyboardEvent): void => {
    if (!menu) return;
    const n = buttons.length;
    if (event.key === 'Escape' || event.key === 'Tab') close();
    else if (event.key === 'ArrowDown' && n) highlight((active + 1) % n);
    else if (event.key === 'ArrowUp' && n) highlight((active - 1 + n) % n);
    else if (event.key === 'Home' && n) highlight(0);
    else if (event.key === 'End' && n) highlight(n - 1);
    else if (event.key === 'Enter' || event.key === ' ') buttons[active]?.click();
    else return;
    event.preventDefault();
    event.stopPropagation();
  };

  const onPointer = (event: Event): void => {
    if (menu && !(event.target instanceof Node && menu.contains(event.target))) close();
  };

  window.addEventListener('contextmenu', onContextMenu);
  window.addEventListener('keydown', onKey, true);
  window.addEventListener('pointerdown', onPointer, true);
  window.addEventListener('blur', close);
  window.addEventListener('resize', close);
  window.addEventListener('wheel', close, { passive: true });
  return () => {
    close();
    window.removeEventListener('contextmenu', onContextMenu);
    window.removeEventListener('keydown', onKey, true);
    window.removeEventListener('pointerdown', onPointer, true);
    window.removeEventListener('blur', close);
    window.removeEventListener('resize', close);
    window.removeEventListener('wheel', close);
  };
}

export { installTooltips } from './tooltips';
