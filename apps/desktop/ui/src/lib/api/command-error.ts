// Błąd komendy rdzenia jako `Error`. `invoke` z Tauri odrzuca obietnicę surowym obiektem `AppError`
// z Rust (`{ code, message }`), a widoki pokazują `error.message` tylko dla `Error` — bez tej zamiany
// użytkownik widział „[object Object]" albo nic.

/** Błąd zgłoszony przez komendę rdzenia: stabilny kod i komunikat PL gotowy do pokazania. */
export class CommandError extends Error {
  constructor(
    readonly code: string,
    message: string,
  ) {
    super(message);
    this.name = 'CommandError';
  }
}

/** Dowolna wartość odrzucenia → `Error` z komunikatem do pokazania. */
export function toError(reason: unknown): Error {
  if (reason instanceof Error) return reason;
  if (typeof reason === 'object' && reason !== null && 'message' in reason) {
    const { message } = reason;
    if (typeof message === 'string') {
      const code = 'code' in reason && typeof reason.code === 'string' ? reason.code : 'internal';
      return new CommandError(code, message);
    }
  }
  return new Error(typeof reason === 'string' ? reason : String(reason));
}

/** Komunikat błędu do pokazania w widoku (toast, alert przy polu). */
export function errorText(reason: unknown): string {
  return toError(reason).message;
}
