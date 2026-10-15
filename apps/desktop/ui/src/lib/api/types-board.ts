// Tablica — przypięte odpowiedzi (komendy `board_*`).

export interface BoardItem {
  readonly id: string;
  readonly session_id: string;
  readonly session_title: string;
  readonly turn_id: string;
  readonly agent: string;
  /** Treść (Markdown) — do kopiowania. */
  readonly text: string;
  /** HTML zsanitowany w Rust — tylko do `SanitizedHtml`. */
  readonly html_sanitized: string;
  readonly pinned_at: string;
}
