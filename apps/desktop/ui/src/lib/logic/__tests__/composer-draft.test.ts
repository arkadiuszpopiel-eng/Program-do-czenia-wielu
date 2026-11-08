// `submitDraft` z ui-kit (Composer): nieudane wysłanie nie kasuje szkicu, udane go nie przywraca
// (uwaga przeglądu PR #1: Q-4). Pakiet ui-kit nie ma własnego runnera testów — logika bez DOM
// jest sprawdzana tutaj.
import { describe, expect, it } from 'vitest';
import { submitDraft, type DraftField } from '@alfa/ui-kit';

const flush = (): Promise<void> => new Promise((resolve) => setTimeout(resolve, 0));

function field(initial: string): DraftField & { value: string } {
  return {
    value: initial,
    get() {
      return this.value;
    },
    set(text: string) {
      this.value = text;
    },
  };
}

describe('submitDraft', () => {
  it('przekazuje treść bez białych znaków na brzegach i czyści pole', () => {
    const f = field('  Cześć  ');
    const sent: string[] = [];
    submitDraft(f, (text) => void sent.push(text));
    expect(sent).toEqual(['Cześć']);
    expect(f.value).toBe('');
  });

  it('odrzucone wysłanie: treść wraca do pustego pola', async () => {
    const f = field('Policz budżet\n');
    submitDraft(f, () => Promise.reject(new Error('rdzeń niedostępny')));
    expect(f.value).toBe('');
    await flush();
    expect(f.value).toBe('Policz budżet\n');
  });

  it('odrzucone wysłanie nie nadpisuje nowej treści wpisanej w międzyczasie', async () => {
    const f = field('Pierwsza');
    submitDraft(f, () => Promise.reject(new Error('błąd')));
    f.value = 'Druga';
    await flush();
    expect(f.value).toBe('Druga');
  });

  it('udane wysłanie: pole zostaje puste (bez zdublowania)', async () => {
    const f = field('Pierwsza');
    submitDraft(f, () => Promise.resolve());
    await flush();
    expect(f.value).toBe('');
  });
});
