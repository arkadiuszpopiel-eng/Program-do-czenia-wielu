// Settings → Commands without a model (keys must match pl-commands.ts).
import type { Message } from './core';
import type { plCommands } from './pl-commands';

export const enCommands: Record<keyof typeof plCommands, Message> = {
  'commands.title': 'Commands without a model',
  'commands.intro':
    'Alfa answers simple commands herself — at once, without a model or cost, offline too. A command must be the whole message; everything else goes to the model.',
  'commands.off': 'Commands are off — every message goes to the model. Turn them on below.',
  'commands.test.title': 'Check a message',
  'commands.test.input': 'Message to check',
  'commands.test.custom': 'Your command “{name}” will answer:',
  'commands.test.builtin': 'A built-in command ({name}) will answer:',
  'commands.test.quiz': 'A quiz will start:',
  'commands.test.model': 'No command matches — the message goes to the model.',
  'commands.test.disabled': 'Commands are off — the message goes to the model.',
  'commands.own.title': 'Your commands',
  'commands.own.hint':
    'Phrase → fixed reply. They come before built-ins; when several match, higher priority wins, then the longer phrase.',
  'commands.own.empty': 'You have no commands of your own yet.',
  'commands.own.add': 'Add command',
  'commands.own.edit': 'Edit',
  'commands.own.remove': 'Remove',
  'commands.own.confirm': 'Remove “{name}”?',
  'commands.own.confirmYes': 'Yes, remove',
  'commands.own.cancel': 'Cancel',
  'commands.own.enabled': 'Command {name} enabled',
  'commands.own.phrases': 'phrases',
  'commands.own.priority': 'priority {n}',
  'commands.mode.exact': 'whole message',
  'commands.mode.starts': 'start of message',
  'commands.mode.contains': 'contains phrase',
  'commands.mode.regex': 'regular expression',
  'commands.form.title': 'Command',
  'commands.form.name': 'Name',
  'commands.form.phrases': 'Phrases (one per line)',
  'commands.form.mode': 'Matching',
  'commands.form.reply': 'Reply',
  'commands.form.priority': 'Priority (−10…10)',
  'commands.form.hint.exact':
    'The message must be exactly the phrase (ignoring case and Polish letters).',
  'commands.form.hint.starts':
    'The message starts with the phrase; put the rest in the reply as {reszta}.',
  'commands.form.hint.contains': 'The phrase anywhere in the message, as whole words.',
  'commands.form.hint.regex':
    'Case-insensitive regular expression; a named group (?P<miasto>…) goes into the reply as {miasto}.',
  'commands.form.save': 'Save',
  'commands.form.cancel': 'Cancel',
  'commands.builtin.title': 'Built-in commands',
  'commands.example': 'Try in the tester: {text}',
};
