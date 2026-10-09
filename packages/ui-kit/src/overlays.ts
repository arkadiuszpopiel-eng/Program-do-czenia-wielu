// Menu kontekstowe i podpowiedzi rysowane przez Alfę zamiast przez system (zgłoszenie właściciela,
// 2026-10-09: po kliknięciu prawym przyciskiem pojawiało się menu WebView2 z „Emoji”, „Kierunek
// pisania”, „Wykonaj inspekcję”; atrybut `title` dawał systemowy dymek). Bez frameworka — te same
// funkcje działają w oknie głównym, Szybkim pytaniu i pigułce. Wygląd: `base.css` (`.alfa-ctx`,
// `.alfa-tip`). Elementy trafiają do warstwy wierzchniej (`popover`), więc są też nad oknami dialogowymi.
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

function show(el: HTMLElement): void {
  try {
    el.showPopover();
  } catch {
    // Starszy silnik bez `popover`: element i tak jest pozycjonowany (`position: fixed`).
    el.dataset.open = '';
  }
}

/** Ustawia element przy punkcie (jednostki okna), nie wychodząc poza okno. */
function place(el: HTMLElement, x: number, y: number, above?: number): void {
  const zoom = uiZoom();
  const [width, height] = [window.innerWidth / zoom, window.innerHeight / zoom];
  const box = el.getBoundingClientRect();
  const left = Math.max(4, Math.min(x, width - box.width - 4));
  const fits = y + box.height + 4 <= height;
  const top = fits ? y : Math.max(4, (above ?? y) - box.height);
  el.style.left = `${left}px`;
  el.style.top = `${top}px`;
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
      const text = await navigator.clipboard.readText();
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

/** Opóźnienie podpowiedzi po najechaniu wskaźnikiem (ms); przy fokusie z klawiatury — od razu. */
const TIP_DELAY_MS = 450;

/**
 * Podpowiedzi Alfy zamiast systemowych dymków: tekst z atrybutu `title` (albo `data-tip`) jest
 * przenoszony do `data-tip`, żeby system nie rysował własnego, i pokazywany jako element w stylu
 * interfejsu — po najechaniu (z krótkim opóźnieniem) albo po wejściu fokusem z klawiatury.
 */
export function installTooltips(): () => void {
  let tip: HTMLElement | null = null;
  let owner: Element | null = null;
  let timer: ReturnType<typeof setTimeout> | undefined;

  const hide = (): void => {
    clearTimeout(timer);
    tip?.remove();
    tip = null;
    owner = null;
  };

  /** Element z podpowiedzią nad wskaźnikiem; przy okazji zdejmuje z niego systemowy `title`. */
  const withTip = (target: EventTarget | null): Element | null => {
    if (!(target instanceof Element)) return null;
    const el = target.closest('[title], [data-tip]');
    if (!el) return null;
    const title = el.getAttribute('title');
    if (title !== null) {
      el.removeAttribute('title');
      if (title) el.setAttribute('data-tip', title);
      else el.removeAttribute('data-tip');
    }
    return el.getAttribute('data-tip') ? el : null;
  };

  const reveal = (el: Element): void => {
    const text = el.getAttribute('data-tip');
    if (!text || !el.isConnected) return;
    tip?.remove();
    const box = document.createElement('div');
    box.className = 'alfa-tip';
    box.setAttribute('role', 'tooltip');
    box.setAttribute('popover', 'manual');
    box.textContent = text;
    document.body.append(box);
    show(box);
    const rect = el.getBoundingClientRect();
    const width = box.getBoundingClientRect().width;
    place(box, rect.left + rect.width / 2 - width / 2, rect.bottom + 6, rect.top - 6);
    tip = box;
  };

  const start = (el: Element, delay: number): void => {
    if (owner === el) return;
    hide();
    owner = el;
    timer = setTimeout(() => reveal(el), delay);
  };

  const onOver = (event: Event): void => {
    const el = withTip(event.target);
    if (el) start(el, TIP_DELAY_MS);
    else if (owner) hide();
  };
  const onFocus = (event: Event): void => {
    const el = withTip(event.target);
    if (el && event.target instanceof Element && event.target.matches(':focus-visible'))
      start(el, 0);
  };

  document.addEventListener('pointerover', onOver);
  document.addEventListener('focusin', onFocus);
  document.addEventListener('focusout', hide);
  document.addEventListener('pointerdown', hide, true);
  document.addEventListener('keydown', hide, true);
  window.addEventListener('wheel', hide, { passive: true });
  window.addEventListener('blur', hide);
  return () => {
    hide();
    document.removeEventListener('pointerover', onOver);
    document.removeEventListener('focusin', onFocus);
    document.removeEventListener('focusout', hide);
    document.removeEventListener('pointerdown', hide, true);
    document.removeEventListener('keydown', hide, true);
    window.removeEventListener('wheel', hide);
    window.removeEventListener('blur', hide);
  };
}
