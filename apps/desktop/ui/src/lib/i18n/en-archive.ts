// Conversation archive in the session list: period and statistics (keys must match pl-archive.ts).
import type { Message } from './core';
import type { plArchive } from './pl-archive';

export const enArchive: Record<keyof typeof plArchive, Message> = {
  'sessions.period': 'Show conversations',
  'sessions.period.all': 'All',
  'sessions.period.today': 'From today',
  'sessions.period.week': 'From the last 7 days',
  'sessions.period.month': 'From the last 30 days',
  'sessions.periodEmpty': 'No conversations in this period.',
  'sessions.stats': 'Messages: {n} · created {date}',
};
