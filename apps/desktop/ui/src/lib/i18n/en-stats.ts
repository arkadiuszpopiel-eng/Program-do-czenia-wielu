// Conversation statistics (keys must match pl-stats.ts).
import type { Message } from './core';
import type { plStats } from './pl-stats';

export const enStats: Record<keyof typeof plStats, Message> = {
  'action.stats': 'Conversation statistics',
  'stats.title': 'Conversation statistics',
  'stats.lead': 'Current conversation path: messages, words, tokens, cost and models.',
  'stats.empty': 'Open a conversation to see its statistics.',
  'stats.list': 'Statistics',
  'stats.messages': 'Messages',
  'stats.messagesValue': '{total} (you: {user}, agents: {agent})',
  'stats.words': 'Words',
  'stats.wordsValue': '{total} (you: {user}, agents: {agent})',
  'stats.characters': 'Characters',
  'stats.reading': 'Reading time',
  'stats.readingValue': 'about {n} min',
  'stats.tokens': 'Tokens',
  'stats.tokensValue': 'input {input}, output {output}',
  'stats.cost': 'Reply cost',
  'stats.free': 'no cost',
  'stats.latency': 'Average reply time',
  'stats.tools': 'Tool uses',
  'stats.period': 'Period',
  'stats.branches': 'Other versions (branches)',
  'stats.models': 'Models',
  'stats.modelsValue': '{name} — {n} replies',
  'stats.none': 'none',
  'stats.copy': 'Copy as Markdown',
  'stats.copied': 'Statistics copied.',
  'stats.close': 'Close',
};
