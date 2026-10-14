// Settings → Model routing (keys must match pl-routing.ts).
import type { Message } from './core';
import type { plRouting } from './pl-routing';

export const enRouting: Record<keyof typeof plRouting, Message> = {
  'routing.intro':
    'Alfa judges how hard a task is and starts with a fitting model: easy — a light, cheap or local one, code and regular work — a medium one (e.g. local Qwen 27B), hard — the strongest (e.g. GPT Sol, Opus). The other models stay as fallback.',
  'routing.settings': 'Routing by difficulty',
  'routing.enabled': 'Match the model to task difficulty',
  'routing.enabledHint': 'Off — Alfa tries models in queue order, as before.',
  'routing.strategy': 'When to reach for a stronger model',
  'routing.strategy.economy': 'Economy',
  'routing.strategy.economyHint': 'strong model only for clearly hard tasks',
  'routing.strategy.balanced': 'Balanced',
  'routing.strategy.balancedHint': 'default',
  'routing.strategy.quality': 'Quality',
  'routing.strategy.qualityHint': 'stronger model already for medium tasks',
  'routing.escalate': 'When the Critic rejects an answer, the fix goes to a stronger model',
  'routing.review': 'The Critic checks answers',
  'routing.review.easy': 'with a light model (cheap)',
  'routing.review.medium': 'with a medium model',
  'routing.review.hard': 'with a strong model',
  'routing.localFirst': 'On the same rung, local model first (no cost)',
  'routing.reorder': 'Also reorder queues you set by hand',
  'routing.reorderHint':
    'Off — when a task class has a queue you set (account or shared), Alfa keeps its order. Escalation after a rejected answer always works.',
  'routing.models': 'Model rungs',
  'routing.modelsHint':
    'The rung is recognised from the model name (size, family). Change it if Alfa got a model wrong.',
  'routing.noModels':
    'The router has no models yet — add an API account or download a local model.',
  'routing.rung.light': 'light',
  'routing.rung.standard': 'medium',
  'routing.rung.strong': 'strong',
  'routing.rung.auto': 'automatic ({rung})',
  'routing.local': 'local',
  'routing.rungFor': 'Rung of model {model}',
  'routing.preview': 'Try an example',
  'routing.previewHint':
    'Type a request — you will see how hard Alfa thinks it is and which model answers first. Nothing is sent to a model.',
  'routing.previewText': 'Example request',
  'routing.previewClass': 'Task class',
  'routing.check': 'Check',
  'routing.level.easy': 'easy',
  'routing.level.medium': 'medium',
  'routing.level.hard': 'hard',
  'routing.result': '{level} task: {reasons}. Starts on the “{rung}” rung.',
  'routing.score': 'Score: {score} of 100.',
  'routing.kept': 'This class has a queue set by hand — Alfa keeps its order.',
  'routing.off': 'No routing by difficulty (off or a fixed-order class) — models in turn.',
  'routing.order': 'Order of attempts',
  'routing.first': 'answers first',
  'routing.empty': 'No model can answer in this class.',
};
