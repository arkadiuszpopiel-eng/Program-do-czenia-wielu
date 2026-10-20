// Arena — comparing two models (keys must match pl-arena.ts).
import type { Message } from './core';
import type { plArena } from './pl-arena';

export const enArena: Record<keyof typeof plArena, Message> = {
  'action.arena': 'Compare models (Arena)',
  'arena.title': 'Compare models (Arena)',
  'arena.lead':
    'The same message goes to two models. Each reply is a new version (branch) of the conversation — nothing is overwritten.',
  'arena.prompt': 'Message',
  'arena.modelA': 'Model A',
  'arena.modelB': 'Model B',
  'arena.same': 'Choose two different models.',
  'arena.costNote':
    'Both replies count towards costs and limits like ordinary messages; the Broker works as always. Replies are generated one after another.',
  'arena.start': 'Compare',
  'arena.cancel': 'Cancel',
  'arena.close': 'Close',
  'arena.stop': 'Stop',
  'arena.loading': 'Loading the model list…',
  'arena.noTarget': 'Send a message first — the Arena compares replies to a user message.',
  'arena.fewModels':
    'At least two models are needed: add an API account (Settings → API accounts and keys) or download a local model.',
  'arena.results': 'Model replies',
  'arena.status.waiting': 'Waiting for its turn',
  'arena.status.streaming': 'Replying…',
  'arena.status.complete': 'Done',
  'arena.status.cancelled': 'Stopped',
  'arena.status.error': 'Error',
  'arena.status.skipped': 'Not started',
  'arena.latency': 'Reply time',
  'arena.tokens': 'Tokens (input / output)',
  'arena.tokensValue': '{input} / {output}',
  'arena.cost': 'Cost',
  'arena.free': 'no cost',
  'arena.faster': 'Faster',
  'arena.cheaper': 'Cheaper',
  'arena.fallback': 'A fallback model replied: {model}',
  'arena.choose': 'Choose this reply',
  'arena.chooseLabel': 'Choose the reply from {model}',
  'arena.chosen': 'Chose the reply from {model}.',
  'arena.answerLabel': 'Reply from {model}',
};
