# ui-kit — SPEC (szkic v0)

## Cel
Jedno źródło prawdy języka wizualnego: tokeny (fonty systemowe, skala typografii, siatka 4 px, promienie, elewacja, neutralne, kolory agentek, semantyczne, ruch, ikony, motywy) i podstawowe komponenty Svelte 5 (Bits UI) ze Storybookiem, regresją wizualną i axe; makiety jako klikalne strony na prawdziwych komponentach (PLAN §14.3, §14.10). Pakiet `packages/ui-kit/`, nie crate Rust.

## Fala i priorytet
F0: v0 (tokeny + podstawowe komponenty + Storybook + makiety 1–3 → akceptacja właściciela, bramka #4). F1: komponenty rozmowy, paneli, ustawień. P0.

## Kontrakt (szkic — TypeScript/CSS, nie Rust)
```ts
// packages/ui-kit — SZKIC
export type Tokens = {
  font: { text: 'Segoe UI Variable', code: 'Cascadia Mono', fallback: 'system-ui' };
  size: 12 | 13 | 14 | 16 | 20 | 24 | 32; space: 4 | 8 | 12 | 16 | 24 | 32 | 48;
  radius: { control: 6; card: 10; overlay: 16; full: 9999 };
  motion: { press: 120; panel: 180; orb: 240; easing: 'ease-out' };
  agent: Record<'alfa' | 'beta' | 'gama' | 'delta', { light: string; dark: string; glyph: 'α' | 'β' | 'γ' | 'δ' }>;
  semantic: { success: string; warning: string; error: string; info: string; risk: { low: string; mid: string; high: string } };
};
// komponenty v0: Button, Input, Textarea (auto-grow), Chip, Avatar(agent), Toast, Dialog(alertdialog), Tabs, Tooltip,
// Panel, ListVirtual, MessageBubble, ToolStep, ApprovalCard, CostChip, StateChip, Orb (Canvas 2D), CommandPalette
```
Zdarzenia magistrali: brak (pakiet UI). Tokeny eksportowane jako CSS custom properties + JSON (dla Rust/`notify` — kolory w toastach nie są potrzebne).

## Zależności
Svelte 5 (runes), Bits UI, Lucide (SVG inline, tree-shaking), Storybook, Playwright (regresja wizualna), axe. Bez fontów webowych, bez Tailwind (do ustalenia w SPEC v1 — PLAN nie rozstrzyga).

## Niezmienniki
- Jeden akcent koloru na raz (kolor mówiącej/pracującej agentki); kolor agentki zawsze w parze z glifem α/β/γ/δ i imieniem.
- Kontrasty: tekst główny ≥ 7:1, pomocniczy ≥ 4,5:1, elementy UI ≥ 3:1; kolory agentek sprawdzane skryptem kontrastu i symulacją deuteranopii, odrębne od semantycznych.
- Ruch tylko `transform`/`opacity`; `prefers-reduced-motion` i ustawienie „bez animacji" wyłączają ruch.
- Motywy: jasny / ciemny / auto / wysoki kontrast (`forced-colors`); gęstość komfortowa/kompaktowa; zoom 80–200%.
- Cele kliknięcia ≥ 24 px; widoczny fokus 2 px; `aria-live` z throttlingiem; karta potwierdzenia jako `alertdialog` z pułapką fokusu.
- Poprawne liczby mnogie PL i żeńskie formy w komponentach tekstowych (i18n od dnia 0).

## Zdolności / uprawnienia
Brak.

## Izolacja
Pakiet TS w WebView2; brak kodu Rust.

## Budżet zasobów
Wkład do JS startowego mieszczący się w ≤ 150 KB gzip łącznie z `ui-shell`; CSS ≤ 30 KB gzip; Orb 30 kl./s z pauzą w tle; wskaźniki głośności ≤ 30 kl./s.

## Konfiguracja (klucze TOML)
Konsumowane z `[ui]` (motyw, gęstość, zoom, akcent = kolor agentki | akcent Windows, `animations = true`); własnych kluczy brak.

## Wkład do UI
Wszystkie komponenty i makiety 1–20 (§14.10) w Storybooku, w wariantach jasny/ciemny i 1280 px.

## Testy akceptacyjne
- `ACC-F0-ui-kit-01`: makiety 1–3 zaakceptowane przez właściciela (bramka ludzka #4).
- `ACC-F0-ui-kit-02`: skrypt kontrastu — wszystkie tokeny kolorów w progach WCAG; deuteranopia — pary agentek rozróżnialne.
- `ACC-F1-ui-kit-03`: Storybook: axe 0 critical/serious; regresja wizualna 0 nieoczekiwanych zmian; `svelte-check` czysty.

## Fake
Storybook z danymi z fixture'ów (rozmowy, kroki narzędzi, karty zatwierdzeń); brak crate `-fake`.

## Otwarte pytania
- Ostateczne odcienie kolorów agentek (koral / mięta / indygo / lazur — propozycja) — z makiet.
- Tailwind vs czysty CSS z tokenami — do ustalenia w SPEC v1 (kryterium: budżet CSS 30 KB).
