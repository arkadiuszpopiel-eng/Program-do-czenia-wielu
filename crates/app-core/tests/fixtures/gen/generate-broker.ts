// Fixture'y komendy `broker_status` — część generatora `generate.ts` (atrapa + `TauriAlfaClient`
// z atrapą `invoke`). Zdarzenia `BrokerStatus` (zerwanie, brak Brokera) — ręcznie w `extra.json`
// (domyślny scenariusz atrapy ich nie emituje).
type Both = (ns: string, method: string, ...args: unknown[]) => Promise<unknown>;

export async function runBroker(both: Both): Promise<void> {
  await both('broker', 'status');
}
