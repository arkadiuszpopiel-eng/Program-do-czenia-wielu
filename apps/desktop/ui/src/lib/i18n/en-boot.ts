// Startup log (keys must match pl-boot.ts).
import type { Message } from './core';
import type { plBoot } from './pl-boot';

export const enBoot: Record<keyof typeof plBoot, Message> = {
  'boot.toast.ok': 'Startup OK · {s} s',
  'boot.toast.warnings': 'Startup with notes · warnings: {w} · {s} s',
  'boot.toast.errors': 'Startup with a problem · errors: {e}, warnings: {w}',
  'boot.showLog': 'Show log',
  'boot.title': 'Startup log',
  'boot.intro':
    'Log entries from program start until startup ends (kernel processes, Broker, watchdog, modules, voice). No conversation content; secrets are masked. The full record is in the log file.',
  'boot.verdict.ok': 'Startup OK',
  'boot.verdict.warnings': 'Startup with notes',
  'boot.verdict.errors': 'Startup with a problem',
  'boot.summary': 'time {s} s · warnings: {w} · errors: {e}',
  'boot.running': 'Startup in progress — entries are still coming in.',
  'boot.onlyProblems': 'Warnings and errors only',
  'boot.copy': 'Copy log',
  'boot.copied': 'Startup log copied.',
  'boot.refresh': 'Refresh',
  'boot.file': 'Log file: {path}',
  'boot.dropped': '{n} further entries skipped (limit 500) — they are in the log file.',
  'boot.empty': 'No entries.',
  'boot.loadFailed': 'Could not read the startup log: {error}',
};
