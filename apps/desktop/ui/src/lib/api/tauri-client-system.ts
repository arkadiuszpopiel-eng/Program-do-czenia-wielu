// `AlfaClient['system']` w programie: komendy rdzenia i dziennik rozruchu (polecenie powłoki).
import { invoke } from '@tauri-apps/api/core';
import type { AlfaClient } from './client';
import { toError } from './command-error';
import { call } from './tauri-call';
import type { BootReport } from './types-boot';

export const systemTauri = (): AlfaClient['system'] => ({
  status: () => call('system_status'),
  retryQueue: () => call('system_retry_queue'),
  clipboardText: () => call('system_clipboard_text'),
  bootReport: () =>
    invoke<BootReport>('boot_report').catch((error: unknown) => {
      throw toError(error);
    }),
});
