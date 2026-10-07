// Atrapa: stan Brokera (komenda `broker_status`, zdarzenie `BrokerStatus`). Domyślnie usługa na
// osobnym koncie z oknem zatwierdzeń i watchdogiem. Scenariusze: `broker-portable` — tryb
// przenośny (słabsza izolacja), `broker-lost` — połączenie zrywa się po 300 ms (bezpieczny stan,
// baner, karta zatwierdzenia nie przenosi do okna), `broker-no-watchdog` — kill-switch awaryjnie
// w aplikacji, `broker-dev` — Broker w procesie (tryb deweloperski, bez okna zatwierdzeń).
import type { AlfaClient } from '../client';
import type { BrokerApi } from '../client-broker';
import type { BrokerStatusView } from '../types-broker';
import type { FakeCore } from './core';

/** Po ilu ms scenariusz `broker-lost` zrywa połączenie. */
export const FAKE_BROKER_LOST_MS = 300;

const SERVICE: BrokerStatusView = {
  mode: 'service',
  state: 'connected',
  approval_window: true,
  watchdog: true,
  isolated: true,
  detail: {
    pl: 'Połączono z usługą Brokera: osobne konto Windows, okno zatwierdzeń z wysoką integralnością (UIPI).',
    en: 'Connected to the Broker service: separate Windows account, approval window with high integrity (UIPI).',
  },
};

const PORTABLE: BrokerStatusView = {
  mode: 'portable',
  state: 'connected',
  approval_window: true,
  watchdog: true,
  isolated: false,
  detail: {
    pl: 'Tryb przenośny: Broker działa jako proces Alfy na Twoim koncie — bez osobnego konta Windows i bez ochrony UIPI okna zatwierdzeń (słabsza izolacja). Pełną izolację daje usługa Brokera.',
    en: 'Portable mode: the Broker runs as an Alfa process under your account — no separate Windows account and no UIPI protection of the approval window (weaker isolation). The Broker service gives full isolation.',
  },
};

const LOST: BrokerStatusView = {
  ...PORTABLE,
  state: 'lost',
  approval_window: false,
  detail: {
    pl: 'Połączenie z Brokerem zerwane (proces Brokera zakończył działanie). Bezpieczny stan: wszystko, co wymaga zgody, jest odrzucane; Alfa ponawia połączenie.',
    en: 'Connection to the Broker lost (the Broker process exited). Safe state: everything that needs approval is denied; Alfa keeps reconnecting.',
  },
};

const DEV: BrokerStatusView = {
  mode: 'in_process',
  state: 'connected',
  approval_window: false,
  watchdog: false,
  isolated: false,
  detail: {
    pl: 'Broker działa w procesie aplikacji bez okna zatwierdzeń (tryb deweloperski): każda prośba o zgodę jest odrzucana.',
    en: 'The Broker runs inside the app process without the approval window (developer mode): every approval request is denied.',
  },
};

export class FakeBrokerStatus {
  private view: BrokerStatusView;

  constructor(private readonly core: FakeCore) {
    const scenario = core.scenario;
    this.view =
      scenario === 'broker-portable' || scenario === 'broker-lost'
        ? PORTABLE
        : scenario === 'broker-no-watchdog'
          ? { ...SERVICE, watchdog: false }
          : scenario === 'broker-dev'
            ? DEV
            : SERVICE;
    if (scenario === 'broker-lost') {
      core.scheduler.setTimeout(() => this.set(LOST), FAKE_BROKER_LOST_MS);
    }
  }

  private set(view: BrokerStatusView): void {
    this.view = view;
    this.core.emit([{ type: 'BrokerStatus', status: view }]);
  }

  get current(): BrokerStatusView {
    return this.view;
  }

  api(): BrokerApi {
    return { status: () => this.core.reply(this.view) };
  }

  /**
   * Karta „czeka na zatwierdzenie" przenosi do okna Brokera tylko, gdy okno działa — inaczej
   * odmowa z wyjaśnieniem (jak `ApprovalWindow::present` w rdzeniu).
   */
  guard(permissions: AlfaClient['permissions']): AlfaClient['permissions'] {
    return {
      ...permissions,
      openApproval: (approvalId) =>
        this.view.approval_window
          ? permissions.openApproval(approvalId)
          : Promise.reject(
              new Error(
                'Ta prośba wymaga potwierdzenia w oknie Brokera, a okno jest niedostępne — bezpieczny stan: prośba zostanie odrzucona.',
              ),
            ),
    };
  }
}
