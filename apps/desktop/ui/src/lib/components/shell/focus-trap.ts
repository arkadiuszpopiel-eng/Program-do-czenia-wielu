// Pułapka fokusu dla szuflad i arkuszy nad treścią (dialog modalny): Tab krąży w obrębie
// elementu, po zamknięciu fokus wraca tam, skąd przyszedł.
const FOCUSABLE =
  'a[href], button:not([disabled]), input:not([disabled]), select:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex="-1"])';

export function focusables(root: HTMLElement): HTMLElement[] {
  return Array.from(root.querySelectorAll<HTMLElement>(FOCUSABLE)).filter(
    (el) => !el.hasAttribute('hidden') && el.getAttribute('aria-hidden') !== 'true',
  );
}

export function focusTrap(node: HTMLElement) {
  const previous = document.activeElement instanceof HTMLElement ? document.activeElement : null;
  requestAnimationFrame(() => (focusables(node)[0] ?? node).focus());

  function onKeydown(event: KeyboardEvent) {
    if (event.key !== 'Tab') return;
    const list = focusables(node);
    const first = list[0];
    const last = list[list.length - 1];
    if (!first || !last) return;
    if (event.shiftKey && document.activeElement === first) {
      event.preventDefault();
      last.focus();
    } else if (!event.shiftKey && document.activeElement === last) {
      event.preventDefault();
      first.focus();
    }
  }

  node.addEventListener('keydown', onKeydown);
  return {
    destroy() {
      node.removeEventListener('keydown', onKeydown);
      if (previous?.isConnected) previous.focus();
    },
  };
}
