import type { AgentId } from '@alfa/ui-kit';

/** Jednorazowy cel skrótu z ikony agentki do jej próbki głosu. */
export const agentNavigation = $state<{ voice: AgentId | null }>({ voice: null });
