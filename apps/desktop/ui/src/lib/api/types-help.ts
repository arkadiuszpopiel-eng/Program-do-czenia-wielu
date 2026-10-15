// Wbudowana instrukcja (komendy `help_*`).

export interface HelpTopic {
  readonly id: string;
  readonly title: string;
}

export interface HelpPage {
  readonly id: string;
  readonly title: string;
  /** HTML zsanitowany w Rust — tylko do `SanitizedHtml`. */
  readonly html_sanitized: string;
}

export interface HelpHit {
  readonly id: string;
  readonly title: string;
  /** Nagłówek sekcji z trafieniem (pusty — przed pierwszym nagłówkiem). */
  readonly section: string;
  readonly snippet: string;
}
