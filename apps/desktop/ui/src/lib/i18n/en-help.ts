// Built-in user guide (keys must match pl-help.ts).
import type { Message } from './core';
import type { plHelp } from './pl-help';

export const enHelp: Record<keyof typeof plHelp, Message> = {
  'action.help': 'Open the user guide',
  'help.title': 'User guide',
  'help.intro':
    'The user guide built into the program works offline. It is written in Polish. Pick a section or search for a word.',
  'help.topic': 'Section',
  'help.search': 'Search the guide',
  'help.results': 'Search results',
  'help.hits': 'Fragments found: {n}',
  'help.noHits': 'Nothing found — try another word.',
  'help.inGuide': 'In the guide',
};
