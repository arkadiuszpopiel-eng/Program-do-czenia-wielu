// Kontrakt rdzenia dopuszcza własne identyfikatory person. Tokeny opisują tylko cztery wbudowane.
import {
  agents,
  agentIds,
  agentHex,
  agentSoftVar,
  agentVar,
  tokens,
  type AgentId,
  type Theme,
} from './tokens';

/** Identyfikator persony działającej w rdzeniu, także własnej z Kreatora. */
export type PersonaId = string;

export interface PersonInfo {
  readonly id: PersonaId;
  readonly name: string;
  readonly glyph: string;
}

/** Sprawdza własny wpis, bez dziedziczonych kluczy obiektu (np. `constructor`). */
export function builtinPerson(id: string): id is AgentId {
  return agentIds.some((known) => known === id);
}

/** Bez katalogu własnej persony pokazujemy jej identyfikator i neutralny symbol. */
export function personInfo(id: string): PersonInfo {
  return builtinPerson(id) ? agents[id] : { id, name: id.slice(0, 64) || '?', glyph: '◇' };
}

/** Nie interpoluje nieznanego identyfikatora w CSS. */
export const personVar = (id: string): string =>
  builtinPerson(id) ? agentVar(id) : 'var(--alfa-color-accent)';
/** Neutralne tło, dopóki nie znamy metadanych własnej persony. */
export const personSoftVar = (id: string): string =>
  builtinPerson(id) ? agentSoftVar(id) : 'var(--alfa-color-surface2)';
/** Bezpieczny kolor Canvas również dla persony spoza tokenów. */
export const personHex = (id: string, theme: Theme): string =>
  builtinPerson(id) ? agentHex(id, theme) : tokens.color.neutral[theme].text;
