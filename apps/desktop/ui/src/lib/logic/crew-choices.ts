import { agentIds, builtinPerson, personInfo, type PersonaId } from '@alfa/ui-kit';
import type { AgentState } from '../api/types';

/** Wbudowane adresatki i własne persony z rolą przypisaną do tej rozmowy. */
export function crewChoices(states: readonly AgentState[]): { id: PersonaId; name: string }[] {
  const custom = states.filter((a) => !builtinPerson(a.id) && a.role_ids.length).map((a) => a.id);
  return [...new Set([...agentIds, ...custom])].map((id) => ({ id, name: personInfo(id).name }));
}
