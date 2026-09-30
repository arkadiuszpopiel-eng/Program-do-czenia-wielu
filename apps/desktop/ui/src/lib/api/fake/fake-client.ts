// FakeAlfaClient: pełna implementacja `AlfaClient` w pamięci — deterministyczna (wirtualny zegar
// w testach), strumień ~100 tokenów/s, scenariusze: offline, 429 z czasem odnowienia, brak kluczy,
// pierwsze uruchomienie, brak mikrofonu, mało miejsca na dysku.
import type { AlfaClient } from '../client';
import type { AlfaEvent } from '../types-system';
import { FakeChat } from './api-chat';
import { accountsApi, transferApi } from './api-hub';
import { sessionsApi } from './api-sessions';
import {
  agentsApi,
  appApi,
  costsApi,
  deviceApi,
  filesApi,
  permissionsApi,
  settingsApi,
  systemApi,
  timelineApi,
  voiceApi,
} from './api-misc';
import { FakeCore, type FakeOptions } from './core';

export { FAKE_SCENARIOS, type FakeScenario, type FakeOptions } from './core';
export { VirtualScheduler } from './scheduler';

export class FakeAlfaClient implements AlfaClient {
  readonly kind = 'fake' as const;
  /** Dostęp do stanu atrapy — dla testów i przełączników w Storybooku. */
  readonly core: FakeCore;
  private readonly chat: FakeChat;

  readonly app: AlfaClient['app'];
  readonly sessions: AlfaClient['sessions'];
  readonly turns: AlfaClient['turns'];
  readonly agents: AlfaClient['agents'];
  readonly costs: AlfaClient['costs'];
  readonly settings: AlfaClient['settings'];
  readonly timeline: AlfaClient['timeline'];
  readonly files: AlfaClient['files'];
  readonly accounts: AlfaClient['accounts'];
  readonly transfer: AlfaClient['transfer'];
  readonly permissions: AlfaClient['permissions'];
  readonly device: AlfaClient['device'];
  readonly voice: AlfaClient['voice'];
  readonly system: AlfaClient['system'];
  readonly quick: AlfaClient['quick'];

  constructor(options: FakeOptions = {}) {
    this.core = new FakeCore(options);
    this.chat = new FakeChat(this.core);
    const { core, chat } = this;
    this.app = appApi(core);
    this.sessions = sessionsApi(core);
    this.turns = chat.turnsApi();
    this.quick = chat.quickApi();
    this.agents = agentsApi(core, chat);
    this.costs = costsApi(core, chat);
    this.settings = settingsApi(core);
    this.timeline = timelineApi(core);
    this.files = filesApi(core);
    this.accounts = accountsApi(core);
    this.transfer = transferApi(core);
    this.permissions = permissionsApi(core);
    this.device = deviceApi(core);
    this.voice = voiceApi(core);
    this.system = systemApi(core, chat);
  }

  subscribe(handler: (batch: readonly AlfaEvent[]) => void): () => void {
    return this.core.subscribe(handler);
  }

  dispose(): void {
    this.core.dispose();
  }

  /** Przełącznik scenariusza „offline" (po powrocie wysyła kolejkę). */
  setOnline(online: boolean): void {
    this.core.setStatus({ online });
    if (online) this.chat.flushQueue();
  }

  /** Przełącznik scenariusza „429" z czasem odnowienia za `minutes` minut. */
  setRateLimited(minutes: number | null): void {
    const resets = minutes === null ? null : this.core.scheduler.now() + minutes * 60_000;
    this.core.setStatus({
      rate_limit:
        resets === null
          ? null
          : { provider: 'Anthropic', resets_at: new Date(resets).toISOString() },
    });
  }
}
