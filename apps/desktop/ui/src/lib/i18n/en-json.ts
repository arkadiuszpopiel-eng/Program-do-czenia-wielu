// Copy and paste lists as JSON (keys must match pl-json.ts).
import type { Message } from './core';
import type { plJson } from './pl-json';

export const enJson: Record<keyof typeof plJson, Message> = {
  'json.copy': 'Copy as JSON',
  'json.copied': 'Copied {n} item(s) as JSON.',
  'json.paste': 'Paste from JSON…',
  'json.field': 'JSON to add (a list or a single object)',
  'json.add': 'Add from JSON',
  'json.cancel': 'Cancel',
  'json.added': 'Added {n} item(s) from JSON.',
  'json.error.json': 'This is not valid JSON.',
  'json.error.shape': 'Item {n} lacks required fields ({fields}).',
  'json.error.empty': 'Nothing to add.',
  'json.error.notObject': 'Item {n} is not a JSON object.',
  'json.error.tooMany': 'Too many items at once (at most {max}).',
};
