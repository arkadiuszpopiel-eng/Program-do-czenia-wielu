// Podpowiedzi Alfy zamiast systemowych dymków — osobny moduł, żeby okna z małym budżetem
// startowym (pigułka głosowa ≤ 8 KB gzip; audyt 2026-10-14, A-11) nie ładowały menu edycji.
import { place, show } from './popover';

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
