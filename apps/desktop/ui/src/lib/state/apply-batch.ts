// Zastosowanie jednego zdarzenia `alfa://events` do stanu okna głównego (wywoływane z paczki
// raz na klatkę): sesje, agentki, koszty, stan systemu, głos, przebiegi agentek, rozmowa.
import type { AlfaEvent } from '../api/types-system';
import { isChatEvent } from '../logic/apply-event';
import type { AppState } from './app.svelte';
import { UNDO_TOAST_MS } from './runs.svelte';

export function applyEvent(app: AppState, event: AlfaEvent): void {
  switch (event.type) {
    case 'SessionUpdated':
      app.sessions.upsert(event.session);
      break;
    case 'SessionRemoved':
      app.sessions.remove(event.session_id);
      break;
    case 'AgentsChanged':
      app.agents[event.session_id] = [...event.agents];
      break;
    case 'ActivityChanged':
      app.activity[event.session_id] = event.activity;
      break;
    case 'CostsChanged':
      if (event.session_id === app.activeId || !event.session_id) app.costs = event.costs;
      break;
    case 'SystemStatusChanged':
      app.system = event.status;
      break;
    case 'MicLevel':
      app.micLevel = event.level;
      app.voice.level = event.level;
      break;
    case 'VoicePill':
      app.micState = event.state.mic;
      app.voice.applyPill(event.state);
      break;
    case 'VoiceStatusChanged':
      app.voice.applyStatus(event.status);
      if (event.status.state !== 'active') app.micState = app.voice.mic;
      break;
    case 'Toast':
      app.toasts.show({ kind: event.kind, message: app.i18n.text(event.message) });
      break;
    case 'OpenSession':
      void app.focusSession(event.session_id);
      break;
    case 'LocalModelProgress':
      app.localDownload = event;
      break;
    case 'AgentRunUpdated':
      app.runs.update(event.run);
      app.notify(event);
      break;
    case 'AgentStep':
    case 'TimelineAppended':
    case 'AccountChanged':
    case 'MemoryChanged':
    case 'TaskUpdated':
    case 'TriggerFired':
      app.notify(event);
      break;
    case 'GuiActivity':
      app.work.applyGui(event.status);
      app.notify(event);
      break;
    case 'HealthChanged':
      app.work.health = { overall: event.overall, pending: event.pending };
      app.notify(event);
      break;
    case 'SkillsChanged':
      app.notify(event);
      break;
    case 'VoiceFeaturesChanged':
      app.voice.applyFeatures(event.features);
      break;
    case 'UpdateStatus':
      app.updates.apply(event.status);
      app.notify(event);
      break;
    case 'BrokerStatus':
      app.broker.apply(event.status);
      break;
    case 'MarshalReportReady':
      app.toasts.show({
        kind: 'info',
        message: app.i18n.t('marshal.reportToast', { text: event.report.text }),
      });
      app.notify(event);
      break;
    default:
      if (!isChatEvent(event)) break;
      app.applyChat(event);
      if (
        event.type === 'ToolCall' &&
        event.session_id === app.activeId &&
        app.runs.shouldToast(event.step)
      ) {
        const { step } = event;
        const token = step.undo_token;
        if (token) {
          app.toasts.show({
            kind: 'info',
            message: step.label,
            actionLabel: app.i18n.t('common.undo'),
            timeoutMs: UNDO_TOAST_MS,
            onAction: () => void app.undoStep(token, step.label),
          });
        }
      }
  }
}
