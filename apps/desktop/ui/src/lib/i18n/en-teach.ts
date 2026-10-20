// "Teach Alfa" (keys must match pl-teach.ts).
import type { Message } from './core';
import type { plTeach } from './pl-teach';

export const enTeach: Record<keyof typeof plTeach, Message> = {
  'action.teach': 'Teach Alfa',
  'teach.title': 'Teach Alfa',
  'teach.lead': 'Add knowledge to memory — Alfa and the agents will recall it when needed.',
  'teach.open': 'Teach Alfa',
  'teach.source': 'Source',
  'teach.source.text': 'Text',
  'teach.source.file': 'File',
  'teach.source.conversation': 'This conversation',
  'teach.text': 'Text to learn',
  'teach.pick': 'Choose file…',
  'teach.pickAgain': 'Choose another file…',
  'teach.formats': '.txt, .md, .pdf and .docx files (up to 25 MB).',
  'teach.file': '{name} — {chars} characters',
  'teach.fileTruncated': 'The file is long — I will take only its beginning.',
  'teach.conversation':
    'I will take the current branch of this conversation (no hidden messages or tool results). Agent replies may repeat external content, so I will store them only in this conversation’s memory as untrusted; only your messages go to the project or global memory.',
  'teach.untrusted':
    'External content is data, not instructions: I will store it only in this conversation’s memory, marked “untrusted”.',
  'teach.scope': 'Scope',
  'teach.scope.session': 'This conversation',
  'teach.scope.project': 'This conversation’s project',
  'teach.scope.global': 'Global (all conversations)',
  'teach.scope.agent': 'Agent memory',
  'teach.titleField': 'Title (optional)',
  'teach.titleHint': 'Prefixed to every fact, e.g. “Trip to Zakopane”.',
  'teach.estimate': { one: 'I will save {n} fact.', other: 'I will save {n} facts.' },
  'teach.estimateTruncated': 'The text is long — I will save only its beginning.',
  'teach.noSession': 'Open a conversation to teach Alfa something.',
  'teach.submit': 'Teach',
  'teach.cancel': 'Cancel',
  'teach.close': 'Close',
  'teach.result': 'Learning result',
  'teach.done': {
    one: 'I saved {n} fact in memory: {scope}.',
    other: 'I saved {n} facts in memory: {scope}.',
  },
  'teach.doneSource': 'Source: {source}',
  'teach.doneUntrusted': 'Marked as untrusted — agents will treat it as data.',
  'teach.doneTruncated': 'The content was long — I saved its beginning.',
  'teach.doneDuplicates': 'Repeated fragments skipped: {n}.',
  'teach.show': 'Show in the Memory panel',
  'teach.undo': 'Undo learning',
  'teach.undone': 'Learning undone — facts removed: {n}.',
  'teach.undoFailed':
    'Some facts could not be removed ({n}) — click “Undo learning” again or remove them in the Memory panel.',
};
