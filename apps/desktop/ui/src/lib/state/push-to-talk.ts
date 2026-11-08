// Spacja „przytrzymaj, aby mówić” (skrót `voice.ptt`, ustawienie „Włączanie mikrofonu” = `ptt`):
// wciśnięcie Spacji w widoku rozmowy poza polem tekstowym i kontrolką włącza mikrofon rdzenia
// (`voice_ptt`), puszczenie — wyłącza. Autopowtórzenie klawisza nic nie wysyła. Utrata fokusu okna
// z wciśniętą Spacją też kończy mówienie (inaczej mikrofon zostałby włączony). W trybie
// „Przełącznik” Spacja działa jak zwykle (przyciski, przewijanie).
import type { AppState } from './app.svelte';
import { attempt } from './attempt';

/** Elementy, w których Spacja ma własne znaczenie (pisanie, naciśnięcie kontrolki). */
const OWN_SPACE =
  'input, textarea, select, button, a[href], summary, [contenteditable="true"], [role="button"], ' +
  '[role="option"], [role="menuitem"], [role="tab"], [role="checkbox"], [role="radio"], [role="switch"]';

function ownsSpace(target: EventTarget | null): boolean {
  const element = target as Partial<Element> | null;
  return typeof element?.closest === 'function' && element.closest(OWN_SPACE) !== null;
}

type KeyLike = Pick<
  KeyboardEvent,
  | 'code'
  | 'repeat'
  | 'ctrlKey'
  | 'altKey'
  | 'shiftKey'
  | 'metaKey'
  | 'isComposing'
  | 'defaultPrevented'
  | 'target'
  | 'preventDefault'
>;

export class PushToTalk {
  /** Spacja wciśnięta i zgłoszona rdzeniowi (czeka na puszczenie). */
  held = false;

  constructor(private readonly app: AppState) {}

  private eligible(event: KeyLike): boolean {
    const { app } = this;
    if (app.view !== 'chat' || app.str('voice.mode', 'toggle') !== 'ptt') return false;
    if (event.ctrlKey || event.altKey || event.shiftKey || event.metaKey) return false;
    return !ownsSpace(event.target);
  }

  down(event: KeyLike): void {
    if (event.code !== 'Space' || event.isComposing) return;
    if (this.held) {
      event.preventDefault();
      return;
    }
    if (event.defaultPrevented || event.repeat || !this.eligible(event)) return;
    event.preventDefault();
    this.held = true;
    void attempt(this.app.toasts, () => this.app.client.voice.ptt(true));
  }

  up(event: Pick<KeyboardEvent, 'code' | 'preventDefault'>): void {
    if (event.code !== 'Space' || !this.held) return;
    event.preventDefault();
    this.release();
  }

  /** Puszczenie bez zdarzenia klawisza (okno straciło fokus). */
  release(): void {
    if (!this.held) return;
    this.held = false;
    void attempt(this.app.toasts, () => this.app.client.voice.ptt(false));
  }
}
