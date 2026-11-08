// Styl interfejsu (`ui.style`): atrybut `data-skin` na <html> — tylko dla znanych stylów.
import { describe, expect, it } from 'vitest';
import { applyAppearance } from '../appearance';
import { applyBootDocument, applySkin, SKINS } from '../window-boot';

/** Minimalny element: testy logiki biegną w Node, bez DOM. */
function fakeRoot() {
  const attrs = new Map<string, string>();
  const props = new Map<string, string>();
  const root = {
    lang: '',
    dataset: {} as Record<string, string>,
    setAttribute: (name: string, value: string) => void attrs.set(name, value),
    removeAttribute: (name: string) => void attrs.delete(name),
    getAttribute: (name: string) => attrs.get(name) ?? null,
    style: {
      setProperty: (name: string, value: string) => void props.set(name, value),
      removeProperty: (name: string) => void props.delete(name),
    },
  };
  return { root: root as unknown as HTMLElement, attrs };
}

describe('applySkin', () => {
  it('ustawia data-skin dla każdego znanego stylu', () => {
    for (const skin of SKINS) {
      const { root, attrs } = fakeRoot();
      applySkin(root, skin);
      expect(attrs.get('data-skin')).toBe(skin);
    }
  });

  it('styl klasyczny i wartości nieznane zdejmują atrybut', () => {
    const { root, attrs } = fakeRoot();
    applySkin(root, 'glass');
    for (const value of ['classic', 'nieznany', '', undefined, null, 7]) {
      attrs.set('data-skin', 'glass');
      applySkin(root, value);
      expect(attrs.has('data-skin')).toBe(false);
    }
  });
});

describe('styl w oknach', () => {
  it('okna poboczne biorą styl z ustawień startowych', () => {
    const { root, attrs } = fakeRoot();
    applyBootDocument(root, { locale: 'pl', settings: { 'ui.theme': 'dark', 'ui.style': 'next' } });
    expect(attrs.get('data-skin')).toBe('next');
    expect(attrs.get('data-theme')).toBe('dark');
  });

  it('okno główne przełącza styl razem z wyglądem', () => {
    const { root, attrs } = fakeRoot();
    const base = { theme: 'auto', density: 'comfortable', zoom: 100, animations: true };
    applyAppearance(root, { ...base, style: 'glass' } as Parameters<typeof applyAppearance>[1]);
    expect(attrs.get('data-skin')).toBe('glass');
    applyAppearance(root, { ...base, style: 'classic' } as Parameters<typeof applyAppearance>[1]);
    expect(attrs.has('data-skin')).toBe(false);
  });
});

describe('styl Junior', () => {
  it('jest znanym stylem i powiększa cały interfejs o 15 %', async () => {
    const { effectiveScale, JUNIOR_SCALE } = await import('../appearance');
    expect(SKINS).toContain('junior');
    expect(JUNIOR_SCALE).toBe(1.15);
    expect(effectiveScale(100, false, 1280, 'junior')).toBeCloseTo(1.15, 5);
    expect(effectiveScale(100, false, 1280, 'classic')).toBe(1);
    expect(effectiveScale(120, false, 1280, 'junior')).toBeCloseTo(1.38, 5);
    // Wąskie okno: powiększenie nie schodzi poniżej 360 px efektywnych (i nigdy nie pomniejsza).
    expect(380 / effectiveScale(100, false, 380, 'junior')).toBeCloseTo(360, 5);
    expect(effectiveScale(100, false, 360, 'junior')).toBe(1);
    expect(effectiveScale(100, false, 300, 'junior')).toBe(1);
    const { root } = fakeRoot();
    applyAppearance(root, {
      theme: 'light',
      style: 'junior',
      density: 'comfortable',
      animations: true,
      zoom: 100,
      fit: false,
      width: 1280,
    });
    expect(root.getAttribute('data-skin')).toBe('junior');
  });
});
