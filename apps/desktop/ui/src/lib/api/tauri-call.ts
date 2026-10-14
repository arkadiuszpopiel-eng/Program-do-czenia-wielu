// Wywołanie komendy rdzenia przez Tauri (wspólne dla `tauri-client*.ts`).
import { invoke } from '@tauri-apps/api/core';
import { toError } from './command-error';
import type { IpcCommandName, IpcCommands } from './generated/ipc';

// Odrzucenie zawsze jako `Error` (rdzeń odsyła `AppError` jako zwykły obiekt).
// Nazwa komendy i nazwy argumentów są sprawdzane w czasie kompilacji z typami generowanymi
// z Rusta (`generated/ipc.ts`): literówka w nazwie albo brak argumentu to błąd typów, nie
// „command not found” w działającym programie.
type CallArgs<K extends IpcCommandName> =
  IpcCommands[K]['args'] extends Readonly<Record<string, never>>
    ? []
    : [args: IpcCommands[K]['args']];
export const call = <T, K extends IpcCommandName = IpcCommandName>(
  command: K,
  ...args: CallArgs<K>
): Promise<T> =>
  invoke<T>(command, args[0]).catch((reason: unknown) => {
    throw toError(reason);
  });
