// WYGENEROWANE z src/tokens/tokens.json przez scripts/build-tokens.mjs — nie edytuj.
/* eslint-disable */

export const tokens = {
  "$comment": "JEDNO źródło prawdy tokenów designu Alfy (PLAN.md §14.3). Z tego pliku `scripts/build-tokens.mjs` generuje src/tokens.css i src/tokens.ts — nie edytuj plików wygenerowanych. Kolory agentek to placeholdery do strojenia na makietach; `scripts/check-contrast.mjs` pilnuje WCAG.",
  "font": {
    "family": {
      "sans": "\"Segoe UI Variable\", \"Segoe UI\", system-ui, sans-serif",
      "mono": "\"Cascadia Mono\", Consolas, ui-monospace, monospace"
    },
    "size": {
      "xs": 12,
      "sm": 13,
      "md": 14,
      "lg": 16,
      "xl": 20,
      "2xl": 24,
      "3xl": 32
    },
    "lineHeight": {
      "text": 1.5,
      "heading": 1.25,
      "tight": 1.2
    },
    "weight": {
      "regular": 400,
      "semibold": 600
    }
  },
  "space": {
    "1": 4,
    "2": 8,
    "3": 12,
    "4": 16,
    "6": 24,
    "8": 32,
    "12": 48
  },
  "radius": {
    "control": 6,
    "card": 10,
    "overlay": 16,
    "full": 9999
  },
  "size": {
    "titlebar": 36,
    "hitTarget": 24,
    "control": 32,
    "sidebarMin": 240,
    "sidebarMax": 320,
    "sidebarRail": 48,
    "panelMin": 300,
    "panelMax": 520,
    "readingColumn": 560,
    "readingColumnWide": 760,
    "focusRing": 2
  },
  "elevation": {
    "light": {
      "1": "0 1px 2px rgba(20, 22, 30, 0.06)",
      "2": "0 2px 8px rgba(20, 22, 30, 0.08), 0 1px 2px rgba(20, 22, 30, 0.04)",
      "3": "0 12px 32px rgba(20, 22, 30, 0.14), 0 2px 6px rgba(20, 22, 30, 0.06)"
    },
    "dark": {
      "1": "none",
      "2": "none",
      "3": "none"
    }
  },
  "color": {
    "neutral": {
      "light": {
        "bg": "#FAFAFB",
        "surface": "#FFFFFF",
        "surface2": "#F1F2F5",
        "surface3": "#E7E9EE",
        "border": "#E2E4EA",
        "borderStrong": "#BDC1CC",
        "text": "#1A1C22",
        "textMuted": "#5B6070",
        "textSubtle": "#626878",
        "textOnAccent": "#FFFFFF",
        "focus": "#2F5BFF",
        "scrim": "rgba(20, 22, 30, 0.32)"
      },
      "dark": {
        "bg": "#131417",
        "surface": "#1B1C21",
        "surface2": "#23252C",
        "surface3": "#2C2F38",
        "border": "#2E313A",
        "borderStrong": "#424652",
        "text": "#EDEEF2",
        "textMuted": "#A7ACBB",
        "textSubtle": "#8A8F9E",
        "textOnAccent": "#131417",
        "focus": "#8FB0FF",
        "scrim": "rgba(0, 0, 0, 0.5)"
      }
    },
    "agent": {
      "alfa": {
        "name": "Alfa",
        "glyph": "α",
        "description": "ciepły koral",
        "light": "#C63D2F",
        "dark": "#F59A84",
        "softLight": "#FBE9E6",
        "softDark": "#3A221E"
      },
      "beta": {
        "name": "Beta",
        "glyph": "β",
        "description": "miętowa zieleń",
        "light": "#0D7A5F",
        "dark": "#4FD9A4",
        "softLight": "#E1F5EE",
        "softDark": "#16302A"
      },
      "gama": {
        "name": "Gama",
        "glyph": "γ",
        "description": "indygo",
        "light": "#4F46E5",
        "dark": "#A5B4FC",
        "softLight": "#E9E8FD",
        "softDark": "#25263F"
      },
      "delta": {
        "name": "Delta",
        "glyph": "δ",
        "description": "lazur / turkus",
        "light": "#0E7490",
        "dark": "#5EEAD4",
        "softLight": "#DFF3F8",
        "softDark": "#163036"
      }
    },
    "semantic": {
      "success": {
        "light": "#127436",
        "dark": "#4ADE80"
      },
      "warning": {
        "light": "#8A5A00",
        "dark": "#FBBF24"
      },
      "error": {
        "light": "#B91C1C",
        "dark": "#F87171"
      },
      "info": {
        "light": "#1D4ED8",
        "dark": "#7DA9FF"
      }
    },
    "risk": {
      "low": {
        "light": "#127436",
        "dark": "#4ADE80"
      },
      "medium": {
        "light": "#8A5A00",
        "dark": "#FBBF24"
      },
      "high": {
        "light": "#B91C1C",
        "dark": "#F87171"
      }
    }
  },
  "motion": {
    "duration": {
      "fast": 120,
      "panel": 180,
      "orb": 240
    },
    "easing": {
      "out": "cubic-bezier(0.2, 0, 0, 1)",
      "inOut": "cubic-bezier(0.4, 0, 0.2, 1)"
    }
  }
} as const;

export type Tokens = typeof tokens;
export type Theme = 'light' | 'dark';
export type AgentId = 'alfa' | 'beta' | 'gama' | 'delta';
export type SemanticColor = 'success' | 'warning' | 'error' | 'info';
export type RiskLevel = 'low' | 'medium' | 'high';
export type FontSize = keyof Tokens['font']['size'];
export type Space = keyof Tokens['space'];
export type Radius = keyof Tokens['radius'];

export const agentIds: readonly AgentId[] = ['alfa', 'beta', 'gama', 'delta'];

export interface AgentMeta {
  readonly id: AgentId;
  readonly name: string;
  readonly glyph: string;
}

export const agents: Readonly<Record<AgentId, AgentMeta>> = {
  alfa: { id: 'alfa', name: 'Alfa', glyph: 'α' },
  beta: { id: 'beta', name: 'Beta', glyph: 'β' },
  gama: { id: 'gama', name: 'Gama', glyph: 'γ' },
  delta: { id: 'delta', name: 'Delta', glyph: 'δ' },
};

/** Zmienna CSS akcentu agentki, np. `var(--alfa-agent-alfa)`. */
export const agentVar = (id: AgentId): string => `var(--alfa-agent-${id})`;
/** Zmienna CSS delikatnego tła agentki. */
export const agentSoftVar = (id: AgentId): string => `var(--alfa-agent-${id}-soft)`;
/** Kolor agentki jako HEX dla danego motywu (np. do Canvas 2D). */
export const agentHex = (id: AgentId, theme: Theme): string => tokens.color.agent[id][theme];
