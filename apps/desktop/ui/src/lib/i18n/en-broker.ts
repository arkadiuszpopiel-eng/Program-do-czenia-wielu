// English dictionary: the Broker (Settings → Permissions and security, safe-state banner,
// "waiting for approval" card). Keys typed from pl-broker.ts.
import type { Message } from './core';
import type { plBroker } from './pl-broker';

export const enBroker: Record<keyof typeof plBroker, Message> = {
  'broker.title': 'Broker (Safety Kernel)',
  'broker.region': 'Broker status',
  'broker.mode.service': 'Broker service under a separate Windows account',
  'broker.mode.portable': 'Portable mode — Broker without a separate account',
  'broker.mode.in_process': 'Broker inside the app process (developer mode)',
  'broker.mode.unavailable': 'No Broker',
  'broker.state.connected': 'Connected',
  'broker.state.connecting': 'Connecting…',
  'broker.state.lost': 'Connection lost — safe state',
  'broker.window.on': 'Approval window: running — you approve every request there.',
  'broker.window.off': 'Approval window: unavailable — approval requests are denied.',
  'broker.watchdog.on':
    'STOP EVERYTHING (Ctrl+Shift+F12): handled by the watchdog, outside the app.',
  'broker.watchdog.off':
    'STOP EVERYTHING (Ctrl+Shift+F12): fallback in the app — the watchdog is not running.',
  'broker.isolation.full': 'Isolation: full (separate account, window protected by UIPI).',
  'broker.isolation.weak': 'Isolation: weaker — the Broker and its window run under your account.',
  'broker.service.hint':
    'The Broker service under a separate Windows account gives full isolation. An administrator installs it once (a single UAC prompt) — see the "Security" guide.',
  'broker.banner.lost':
    'Connection to the Broker lost. Safe state: everything that needs approval is denied; Alfa keeps reconnecting.',
  'broker.banner.unavailable':
    'No isolated Broker — agents will not do anything that needs approval. Reinstall Alfa.',
  'broker.banner.connecting':
    'Connecting to the Broker… approval requests are denied until connected.',
  'broker.banner.watchdog':
    'The watchdog is not running — STOP EVERYTHING (Ctrl+Shift+F12) is handled by the app as a fallback.',
  'broker.banner.dev': 'Developer mode: Broker inside the app process, no approval window.',
  'broker.banner.details': 'Details',
  'broker.approval.lost':
    'The Broker is unavailable — this request cannot be approved now; the agent will be denied.',
};
