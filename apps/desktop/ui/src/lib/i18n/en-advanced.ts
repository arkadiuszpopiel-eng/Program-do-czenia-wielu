// Settings → Advanced (keys must match pl-advanced.ts).
import type { Message } from './core';
import type { plAdvanced } from './pl-advanced';

export const enAdvanced: Record<keyof typeof plAdvanced, Message> = {
  'advanced.title': 'Raw configuration',
  'advanced.intro':
    'Edit the configuration files directly, in TOML. The shared file applies on every computer, the overlay — only on this one. Before saving, Alfa checks the syntax and every key; if something is wrong, the file stays unchanged.',
  'advanced.rules':
    'Kernel settings (approvals, supervision, blocks) cannot be changed here — only through the approval window. Do not put API keys or passwords in files: they belong in Windows Credential Manager.',
  'advanced.problems': 'Problems with configuration files',
  'advanced.layer': 'File',
  'advanced.layer.shared': 'Shared',
  'advanced.layer.machine': 'This computer',
  'advanced.file': 'File on disk',
  'advanced.editor': 'Contents: {file}',
  'advanced.check': 'Check',
  'advanced.checkOk': 'Syntax OK · keys: {n}',
  'advanced.save': 'Save',
  'advanced.saved': 'Configuration saved and loaded.',
  'advanced.revert': 'Discard changes',
  'advanced.unsaved': 'Unsaved changes',
};
