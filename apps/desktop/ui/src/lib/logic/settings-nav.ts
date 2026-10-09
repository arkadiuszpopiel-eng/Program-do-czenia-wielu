// Nawigacja Ustawień: trafienia wyszukiwarki per strona, ruch klawiaturą po liście i zapamiętane
// zwinięcie grup. Czysta logika — komponent `SettingsNav` tylko ją wyświetla.

/** Liczba pasujących ustawień per strona; strona trafiona samą nazwą ma wpis z zerem. */
export function hitsByPage(
  settingHits: readonly { readonly page: { readonly id: string } }[],
  pageHits: readonly { readonly id: string }[],
): Map<string, number> {
  const out = new Map<string, number>();
  for (const page of pageHits) out.set(page.id, 0);
  for (const hit of settingHits) out.set(hit.page.id, (out.get(hit.page.id) ?? 0) + 1);
  return out;
}

/** Strona, na którą przenosi klawisz (strzałki, Home, End); `null`, gdy klawisz nie nawiguje. */
export function nextPage(ids: readonly string[], current: string, key: string): string | null {
  if (ids.length === 0) return null;
  const at = ids.indexOf(current);
  const pick = (index: number) => ids[Math.max(0, Math.min(ids.length - 1, index))] ?? null;
  switch (key) {
    case 'ArrowDown':
      return pick(at + 1);
    case 'ArrowUp':
      return pick(at < 0 ? 0 : at - 1);
    case 'Home':
      return pick(0);
    case 'End':
      return pick(ids.length - 1);
    default:
      return null;
  }
}

const COLLAPSED_KEY = 'alfa.settings.nav.collapsed';

/** Zwinięte grupy z poprzedniego otwarcia (preferencja tego okna; brak pamięci = nic zwiniętego). */
export function readCollapsed(storage: Pick<Storage, 'getItem'> | null): string[] {
  try {
    const raw: unknown = JSON.parse(storage?.getItem(COLLAPSED_KEY) ?? '[]');
    return Array.isArray(raw) ? raw.filter((id): id is string => typeof id === 'string') : [];
  } catch {
    return [];
  }
}

export function writeCollapsed(storage: Pick<Storage, 'setItem'> | null, ids: readonly string[]) {
  try {
    storage?.setItem(COLLAPSED_KEY, JSON.stringify([...ids].sort()));
  } catch {
    // Pamięć przeglądarki niedostępna — zwinięcie działa do zamknięcia okna.
  }
}

export function toggled(ids: readonly string[], id: string): string[] {
  return ids.includes(id) ? ids.filter((other) => other !== id) : [...ids, id];
}
