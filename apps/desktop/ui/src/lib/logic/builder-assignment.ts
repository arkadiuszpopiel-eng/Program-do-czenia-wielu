import type { AlfaClient } from '../api/client';

/** Zmienia tylko wskazaną rolę; odczytuje bieżącą obsadę, zamiast nadpisywać ją kopią z UI. */
export async function setBuilderRole(
  client: Pick<AlfaClient, 'agents'>,
  session: string,
  persona: string,
  role: string,
  enabled: boolean,
) {
  const current = await client.agents.list(session);
  const roles = new Set(current.find((a) => a.id === persona)?.role_ids ?? []);
  if (enabled) roles.add(role);
  else roles.delete(role);
  await client.agents.setRoles(session, persona, [...roles]);
  return client.agents.list(session);
}
