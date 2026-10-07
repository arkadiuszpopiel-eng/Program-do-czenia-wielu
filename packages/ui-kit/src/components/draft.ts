// Wysłanie szkicu z pola composera (logika bez DOM, testowana w aplikacji): pole czyszczone od razu,
// a gdy wywołujący zwróci odrzuconą obietnicę (wysłanie nie doszło do skutku), treść wraca —
// o ile użytkownik nie zaczął w tym czasie pisać od nowa. Błąd zgłasza wywołujący.

/** Pole z treścią (np. `bind:value` komponentu). */
export interface DraftField {
  get(): string;
  set(text: string): void;
}

/** Przekazuje treść (bez białych znaków na brzegach) i czyści pole; odrzucenie ją przywraca. */
export function submitDraft(
  field: DraftField,
  onsubmit: ((text: string) => unknown) | undefined,
): void {
  const draft = field.get();
  const sent = onsubmit?.(draft.trim());
  field.set('');
  if (sent instanceof Promise) {
    sent.catch(() => {
      if (field.get() === '') field.set(draft);
    });
  }
}
