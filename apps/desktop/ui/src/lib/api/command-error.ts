// Błąd komendy rdzenia jako `Error`. `invoke` z Tauri odrzuca obietnicę surowym obiektem `AppError`
// z Rust (`{ code, message, details? }`), a widoki pokazują `error.message` tylko dla `Error` — bez tej zamiany
// użytkownik widział „[object Object]" albo nic.

/** Szczegóły błędu do odczytu przez program: stałe klucze, wartości to tekst, liczba albo flaga. */
export type ErrorDetails = Readonly<Record<string, string | number | boolean>>;

const NO_DETAILS: ErrorDetails = Object.freeze({});

/**
 * Błąd zgłoszony przez komendę rdzenia: stabilny kod, komunikat PL gotowy do pokazania
 * i szczegóły (`kind`, `id`, `field`, `reason`, `module`, `provider`, `retry_after_s`) — widok
 * rozróżnia po nich przypadki bez dopasowywania tekstu komunikatu.
 */
export class CommandError extends Error {
  constructor(
    readonly code: string,
    message: string,
    readonly details: ErrorDetails = NO_DETAILS,
  ) {
    super(message);
    this.name = 'CommandError';
  }

  /** Tekstowy szczegół (np. `kind`, `field`) albo `null`. */
  detail(key: string): string | null {
    const value = this.details[key];
    return typeof value === 'string' ? value : null;
  }

  /** Po ilu sekundach ponowić (limit dostawcy), jeśli rdzeń to podał. */
  get retryAfterSeconds(): number | null {
    const value = this.details.retry_after_s;
    return typeof value === 'number' && value >= 0 ? value : null;
  }
}

/** Słownik szczegółów z ładunku błędu: tylko proste wartości, reszta jest pomijana. */
function readDetails(raw: unknown): ErrorDetails {
  if (typeof raw !== 'object' || raw === null) return NO_DETAILS;
  const out: Record<string, string | number | boolean> = {};
  for (const [key, value] of Object.entries(raw)) {
    if (typeof value === 'string' || typeof value === 'number' || typeof value === 'boolean')
      out[key] = value;
  }
  return out;
}

/** Dowolna wartość odrzucenia → `Error` z komunikatem do pokazania. */
export function toError(reason: unknown): Error {
  if (reason instanceof Error) return reason;
  if (typeof reason === 'object' && reason !== null && 'message' in reason) {
    const { message } = reason;
    if (typeof message === 'string') {
      const code = 'code' in reason && typeof reason.code === 'string' ? reason.code : 'internal';
      const details = 'details' in reason ? readDetails(reason.details) : NO_DETAILS;
      return new CommandError(code, message, details);
    }
  }
  return new Error(typeof reason === 'string' ? reason : String(reason));
}

/** Komunikat błędu do pokazania w widoku (toast, alert przy polu). */
export function errorText(reason: unknown): string {
  return toError(reason).message;
}
