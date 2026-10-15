// Prompt templates: Settings and the “Insert template” window (keys must match pl-templates.ts).
import type { Message } from './core';
import type { plTemplates } from './pl-templates';

export const enTemplates: Record<keyof typeof plTemplates, Message> = {
  'action.templates': 'Insert prompt template',
  'tpl.fromMessage': 'Save as prompt template',
  'tpl.savedFrom': 'Saved template “{name}” (Ctrl+K → “Insert prompt template”).',
  'tpl.defaultName': 'Template',
  'tpl.tooLong': 'The message is too long for a template (at most {max} characters).',
  'tpl.full': 'You already have {max} templates — remove one in Settings → Prompt templates.',
  'tpl.title': 'Prompt templates',
  'tpl.intro':
    'Ready prompts for the agents with variables in double braces, e.g. {{adresat}}. When inserting, Alfa asks for the values and puts the finished text in the message field — you send it yourself. You can also open the window from the Ctrl+K palette: “Insert prompt template”.',
  'tpl.empty': 'You have no templates yet.',
  'tpl.list': 'Templates',
  'tpl.vars': 'variables',
  'tpl.use': 'Use',
  'tpl.edit': 'Edit',
  'tpl.remove': 'Remove',
  'tpl.confirm': 'Remove “{name}”?',
  'tpl.confirmYes': 'Yes, remove',
  'tpl.cancel': 'Cancel',
  'tpl.add': 'Add template',
  'tpl.form.title': 'Template',
  'tpl.form.name': 'Name',
  'tpl.form.text': 'Text',
  'tpl.form.hint':
    'A variable is a name in double braces: {{temat}} (letters, digits, space, _ and -; at most 10).',
  'tpl.form.save': 'Save',
  'tpl.dialog.title': 'Insert template',
  'tpl.dialog.lead': 'Fill in the variables — empty ones stay in the text for you to edit.',
  'tpl.dialog.empty': 'You have no templates yet — add them in Settings → Prompt templates.',
  'tpl.dialog.pick': 'Template',
  'tpl.dialog.preview': 'Preview',
  'tpl.dialog.insert': 'Insert into conversation',
  'tpl.dialog.cancel': 'Cancel',
  'tpl.dialog.manage': 'Manage templates',
};
