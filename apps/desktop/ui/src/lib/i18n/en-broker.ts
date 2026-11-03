// English dictionary: the Broker (Settings → Permissions and security, safe-state banner,
// "waiting for approval" card). Keys typed from pl-broker.ts.
import type { Message } from './core';
import type { plBroker } from './pl-broker';

export const enBroker: Record<keyof typeof plBroker, Message> = {
  'broker.title': 'Shadow Judge (Safety Kernel)',
  'broker.region': 'Shadow Judge status',
  'broker.mode.service': 'Shadow Judge service under a separate Windows account',
  'broker.mode.portable': 'Portable mode — Shadow Judge without a separate account',
  'broker.mode.in_process': 'Shadow Judge inside the app process (developer mode)',
  'broker.mode.unavailable': 'No Shadow Judge',
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
  'broker.isolation.weak':
    'Isolation: weaker — the Shadow Judge and its window run under your account.',
  'broker.service.hint':
    'The Shadow Judge service under a separate Windows account gives full isolation. An administrator installs it once (a single UAC prompt) — see the "Security" guide.',
  'broker.banner.lost':
    'Connection to the Shadow Judge lost. Safe state: everything that needs approval is denied; Alfa keeps reconnecting.',
  'broker.banner.unavailable':
    'No isolated Shadow Judge — agents will not do anything that needs approval. Reinstall Alfa.',
  'broker.banner.connecting':
    'Connecting to the Shadow Judge… approval requests are denied until connected.',
  'broker.banner.watchdog':
    'The watchdog is not running — STOP EVERYTHING (Ctrl+Shift+F12) is handled by the app as a fallback.',
  'broker.banner.dev': 'Developer mode: Shadow Judge inside the app process, no approval window.',
  'broker.banner.unknown':
    'Could not read the Shadow Judge state — it is unknown whether the approval window works. Check Details or restart Alfa.',
  'broker.banner.details': 'Details',
  'broker.approval.lost':
    'The Shadow Judge is unavailable — this request cannot be approved now; the agent will be denied.',
  // Tryb pracy z paska tytułu (`ModeMenu`, decyzja 27.10).
  'titlebar.mode': 'Work mode: {mode}. Change mode',
  'titlebar.modeTip': 'Agents’ work mode — click to change (1–5)',
  'mode.title': 'Work mode',
  'status.changeMode': 'Change work mode',
  'mode.L0.name': 'Plan',
  'mode.L0.desc': 'Plan first, changes after you accept it. This conversation only.',
  'mode.L1.name': 'Manual',
  'mode.L1.desc': 'Every change needs your "yes".',
  'mode.L2.name': 'Accept edits',
  'mode.L2.desc':
    'Changes in the work folder without asking; files outside it and the Internet — asks.',
  'mode.L3.name': 'Automatic',
  'mode.L3.desc': 'Works on her own; asks only about irreversible things outside the scope.',
  'mode.L4.name': 'Full access',
  'mode.L4.desc': 'No questions. Choose once — it stays after a restart.',
  'mode.note':
    'Applies to all conversations. You confirm a raise once in the Shadow Judge window. On Full access the Shadow Judge window never shows until you lower the mode. Always kept: Ctrl+Shift+F12, the journal and the Kernel’s hard blocks.',
  'mode.more': 'More in Settings → Permissions',
  'mode.applied': 'Work mode: {mode}.',
  'mode.requested': 'Confirm "{mode}" in the Shadow Judge window — once is enough.',
  'mode.raised': 'Work mode: {mode} — approved and remembered.',
  'mode.notApplied': 'The Shadow Judge did not accept "{mode}" — "{current}" is in effect.',
  'mode.sessionOwn':
    'This conversation has its own mode: {mode}. The choice here changes the other conversations; change this one in Permissions.',
  'mode.badge': 'Full access is on — agents act without asking. Click to change the work mode',
  // Denial with "tell the agent what to do instead" (plan 50 §4.2).
  'approval.feedback.open': 'Tell the agent what to do instead',
  'approval.feedback.label': 'What should the agent do instead?',
  'approval.feedback.placeholder': 'e.g. do not delete — move the files to the Archive',
  'approval.feedback.send': 'Send to the agent',
  'approval.feedback.sent': 'The agent got the hint — she will use it in the next step.',
  'approval.feedback.toComposer':
    'The task has already finished — the hint is waiting in the message box; send it with Enter.',
  // "Plan" mode (plan 50 §4.1, Z.5): three answers below the plan.
  'plan.title': 'Plan ready — what next?',
  'plan.auto': 'Yes, and work automatically',
  'plan.manual': 'Yes, I approve manually',
  'plan.refine': 'No, keep planning',
  'plan.requested':
    'Confirm "{mode}" in the Shadow Judge window — then the agent carries out the plan.',
  'plan.started': 'Plan accepted — the agent carries it out in "{mode}" mode.',
  'plan.refineHint': 'Write what to change in the plan — the agent will keep planning.',
  'plan.go': 'I accept the plan — carry it out.',
  // "Max access" (plan 50 §8.1).
  'maxAccess.name': 'Max access',
  'maxAccess.desc':
    'No questions at all for agents on the chosen models. You confirm turning it on once in the Shadow Judge window.',
  'maxAccess.onDesc':
    'On: {where} — agents ask about nothing. STOP (Ctrl+Shift+F12), the journal and the Kernel’s hard blocks stay.',
  'maxAccess.scopeLabel': 'Max access scope',
  'maxAccess.scope.local': 'Local',
  'maxAccess.scope.cloud': 'Cloud',
  'maxAccess.scope.all': 'All',
  'maxAccess.requested':
    'Confirm "Max access" ({where}) in the Shadow Judge window — once is enough.',
  'maxAccess.off': 'Max access is off — agents ask again according to the work mode.',
  'maxAccess.badge':
    'Max access is on ({where}) — agents act without any questions. Click to change',
  'broker.grants.title': '“Always in this scope” permissions',
  'broker.grants.intro':
    'You grant them in the Shadow Judge window and choose how long: 1 h, 3 h, a day or until revoked. A permission covers one session and one kind of action; it does not change the work mode. Revoking takes effect at once, and STOP (Ctrl+Shift+F12) clears all permissions.',
  'broker.grants.empty': 'No permissions — the Shadow Judge will ask about every such action.',
  'broker.grants.until': 'until {when}',
  'broker.grants.untilRevoked': 'until revoked',
  'broker.grants.allAgents': 'All agents in the session',
  'broker.grants.revoke': 'Revoke',
  'broker.grants.revokeLabel': 'Revoke permission: {scope}',
  'broker.grants.revoked': 'Permission revoked — the Shadow Judge will ask again.',
  'broker.grants.gone':
    'That permission was already gone (expired or revoked earlier) — list refreshed.',
  'broker.grants.readFailed':
    'Could not read permissions from the Shadow Judge — the list may be out of date. Retrying shortly.',
  'child.title': 'Child profile',
  'child.intro':
    'With the profile on, the child can use only the Plan and Manual modes. Accept edits, Automatic, Full access, Max access and standing permissions are behind a padlock that only the parent PIN opens, for a set time. The Shadow Judge checks the PIN and keeps only its hash; after 5 wrong tries the PIN locks for 15 minutes.',
  'child.state.none': 'No parent PIN yet — set one first.',
  'child.state.off': 'Child profile is off — work modes have no padlock.',
  'child.state.locked': 'Child profile is on — padlock closed.',
  'child.state.open': 'Padlock open {until}.',
  'child.until.close': 'until Alfa closes',
  'child.until.time': 'until {when}',
  'child.pin': 'Parent PIN',
  'child.oldPin': 'Current PIN',
  'child.newPin': 'New PIN (4–8 digits)',
  'child.repeat': 'Repeat the new PIN',
  'child.setPin': 'Set PIN',
  'child.changePin': 'Change PIN',
  'child.enable': 'Turn on the child profile',
  'child.disable': 'Turn off the child profile',
  'child.open': 'Open the padlock',
  'child.close': 'Close the padlock',
  'child.for': 'For how long',
  'child.for.15': '15 min',
  'child.for.60': '1 h',
  'child.for.close': 'Until Alfa closes',
  'child.mismatch': 'The new PINs differ.',
  'child.format': 'The PIN has 4 to 8 digits.',
  'child.attempts': 'PIN tries left: {n}.',
  'child.blocked': 'Too many wrong tries — the PIN is locked until {when}.',
  'child.pinSet': 'Parent PIN saved.',
  'child.enabled': 'Child profile on — padlock closed.',
  'child.disabled': 'Child profile off.',
  'child.opened': 'Padlock open {until}.',
  'child.closed': 'Padlock closed.',
  'child.askParent': 'Ask a parent',
  'child.lockedNote':
    'Parent padlock — higher modes open with the parent PIN in Settings → Permissions.',
  'child.badge': 'Child profile — padlock closed. Click to see the modes',
  'child.badgeWord': 'Child',
  'parentLog.title': 'Parent log',
  'parentLog.intro':
    'What happened in Alfa: conversation topics, agent actions (tools, websites, files), permissions, refusals and padlock attempts. Conversation content is not shown here. The parent PIN opens the log.',
  'parentLog.show': 'Show the log',
  'parentLog.hide': 'Hide the log',
  'parentLog.empty': 'No recorded conversations.',
  'parentLog.loading': 'Loading the log…',
  'parentLog.failed': 'Could not load the log — try again.',
  'parentLog.session': 'Conversation “{title}” · last {when}',
  'parentLog.none': 'no agent actions',
  'child.contractual':
    'Contractual protection: the Shadow Judge now runs on your Windows account, so someone with access to it could delete the padlock record. Full protection needs the Shadow Judge as a Windows service on a separate account.',
};
