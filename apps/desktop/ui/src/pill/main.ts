// Pigułka głosowa (ui-quick, makieta 4): minimalna strona BEZ frameworka (≤ 8 KB gzip).
// Awatar mówiącej agentki, fala głośności (≤ 30 kl./s, pauza gdy okno ukryte), stan mikrofonu
// (ikona + tekst, nigdy sam kolor), kto mówi i transkrypt częściowy użytkownika, przyciski Stop
// i Wycisz. Dane: zdarzenia `VoicePill`/`MicLevel`.
import '@alfa/ui-kit/tokens.css';
import './pill.css';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import type { AppBootstrap } from '../lib/api/types-system';
import { applyBootDocument } from '../lib/window-boot';

type Agent = 'alfa' | 'beta' | 'gama' | 'delta';
type Mic = 'off' | 'listening' | 'hearing' | 'processing' | 'speaking' | 'muted' | 'dnd';
type Speaker = 'nobody' | 'user' | 'agent';
type PillEvent =
  | {
      type: 'VoicePill';
      state: { agent: Agent; mic: Mic; level: number; speaker?: Speaker; partial?: string | null };
    }
  | { type: 'MicLevel'; level: number };

const GLYPH: Record<Agent, string> = { alfa: 'α', beta: 'β', gama: 'γ', delta: 'δ' };
const NAME: Record<Agent, string> = { alfa: 'Alfa', beta: 'Beta', gama: 'Gama', delta: 'Delta' };
const TEXT = {
  pl: {
    off: 'Mikrofon wyłączony',
    listening: 'Słucha',
    hearing: 'Słyszy Cię',
    processing: 'Przetwarza',
    speaking: 'mówi',
    muted: 'Wyciszony',
    dnd: 'Nie przeszkadzać',
    stop: 'Zatrzymaj mowę',
    mute: 'Wycisz',
    unmute: 'Włącz mikrofon',
    label: 'Pigułka głosowa',
    title: 'Alfa — pigułka głosowa',
  },
  en: {
    off: 'Microphone off',
    listening: 'Listening',
    hearing: 'Hearing you',
    processing: 'Processing',
    speaking: 'speaking',
    muted: 'Muted',
    dnd: 'Do not disturb',
    stop: 'Stop speech',
    mute: 'Mute',
    unmute: 'Unmute',
    label: 'Voice pill',
    title: 'Alfa — voice pill',
  },
} as const;
const ICON: Record<Mic, string> = {
  off: '○',
  listening: '●',
  hearing: '◉',
  processing: '◌',
  speaking: '▶',
  muted: '⊘',
  dnd: '☾',
};

// Ikony liniowe (kształty z zestawu Lucide) budowane przez DOM — bez innerHTML.
type Shape = readonly [tag: string, attrs: Readonly<Record<string, string>>];
const SQUARE: readonly Shape[] = [['rect', { width: '14', height: '14', x: '5', y: '5', rx: '2' }]];
const MIC: readonly Shape[] = [
  ['path', { d: 'M12 19v3' }],
  ['path', { d: 'M19 10v2a7 7 0 0 1-14 0v-2' }],
  ['rect', { x: '9', y: '2', width: '6', height: '13', rx: '3' }],
];
const MIC_OFF: readonly Shape[] = [
  ['path', { d: 'M2 2l20 20' }],
  ['path', { d: 'M18.89 13.23A7.12 7.12 0 0 0 19 12v-2' }],
  ['path', { d: 'M5 10v2a7 7 0 0 0 12 5' }],
  ['path', { d: 'M15 9.34V5a3 3 0 0 0-5.68-1.33' }],
  ['path', { d: 'M9 9v3a3 3 0 0 0 5.12 2.12' }],
  ['path', { d: 'M12 19v3' }],
];

function icon(shapes: readonly Shape[]): SVGSVGElement {
  const ns = 'http://www.w3.org/2000/svg';
  const svg = document.createElementNS(ns, 'svg');
  for (const [k, v] of Object.entries({
    viewBox: '0 0 24 24',
    width: '16',
    height: '16',
    fill: 'none',
    stroke: 'currentColor',
    'stroke-width': '1.5',
    'stroke-linecap': 'round',
    'stroke-linejoin': 'round',
    'aria-hidden': 'true',
  }))
    svg.setAttribute(k, v);
  for (const [tag, attrs] of shapes) {
    const node = document.createElementNS(ns, tag);
    for (const [k, v] of Object.entries(attrs)) node.setAttribute(k, v);
    svg.append(node);
  }
  return svg;
}

const tauri = '__TAURI_INTERNALS__' in window;
// Język i motyw (`ui.theme`) z `app_bootstrap` przed pierwszym renderem; podgląd poza Tauri —
// `?lang=`. Błąd odczytu nie blokuje pigułki: zostaje polski i motyw systemu.
const boot = tauri ? await invoke<AppBootstrap>('app_bootstrap').catch(() => null) : null;
const wanted = boot?.locale ?? new URLSearchParams(location.search).get('lang');
const lang = wanted === 'en' ? 'en' : 'pl';
const L = TEXT[lang];
applyBootDocument(document.documentElement, { locale: lang, settings: boot?.settings ?? {} });
document.title = L.title;
const state = {
  agent: 'beta' as Agent,
  mic: 'speaking' as Mic,
  level: 0,
  speaker: 'agent' as Speaker,
  partial: null as string | null,
};

// Podgląd poza Tauri (przeglądarka, testy E2E): stan z adresu, np. `?mic=hearing&partial=…`.
if (!tauri) {
  const query = new URLSearchParams(location.search);
  const mic = query.get('mic');
  if (mic && mic in ICON) state.mic = mic as Mic;
  const partial = query.get('partial');
  if (partial) {
    state.speaker = 'user';
    state.partial = partial.slice(0, 120);
  }
}

const root = document.getElementById('pill');
if (!root) throw new Error('Brak elementu #pill');
root.setAttribute('aria-label', L.label);

function el<K extends keyof HTMLElementTagNameMap>(
  tag: K,
  cls: string,
  text = '',
): HTMLElementTagNameMap[K] {
  const node = document.createElement(tag);
  node.className = cls;
  node.textContent = text;
  return node;
}

const avatar = el('span', 'avatar');
avatar.setAttribute('aria-hidden', 'true');
const wave = el('span', 'wave');
wave.setAttribute('aria-hidden', 'true');
const bars = Array.from({ length: 5 }, () => wave.appendChild(el('span', 'bar')));
const status = el('span', 'status');
status.setAttribute('role', 'status');
const stop = el('button', 'btn');
const mute = el('button', 'btn');
stop.append(icon(SQUARE));
for (const b of [stop, mute]) b.type = 'button';
root.append(avatar, wave, status, stop, mute);

function render(): void {
  root?.style.setProperty('--accent', `var(--alfa-agent-${state.agent})`);
  avatar.textContent = GLYPH[state.agent];
  const label = state.mic === 'speaking' ? `${NAME[state.agent]} ${L.speaking}` : L[state.mic];
  // Transkrypt częściowy (szary w pełnym trybie) — w pigułce w cudzysłowie po stanie.
  const partial = state.speaker === 'user' && state.partial ? ` „${state.partial}”` : '';
  status.textContent = `${ICON[state.mic]} ${label}${partial}`;
  status.title = status.textContent;
  stop.setAttribute('aria-label', L.stop);
  stop.title = L.stop;
  const muted = state.mic === 'muted';
  mute.replaceChildren(icon(muted ? MIC_OFF : MIC));
  mute.setAttribute('aria-label', muted ? L.unmute : L.mute);
  mute.setAttribute('aria-pressed', String(muted));
  mute.title = muted ? L.unmute : L.mute;
}

let frame = 0;
let last = 0;
function draw(now: number): void {
  frame = requestAnimationFrame(draw);
  if (now - last < 33) return; // ≤ 30 kl./s
  last = now;
  if (!tauri) state.level = Math.max(0, Math.sin(now / 180) * 0.5 + Math.sin(now / 67) * 0.3);
  bars.forEach((bar, i) => {
    const shape = 1 - Math.abs(i - 2) / 3;
    bar.style.transform = `scaleY(${(0.15 + state.level * shape).toFixed(3)})`;
  });
}

function animate(on: boolean): void {
  cancelAnimationFrame(frame);
  if (on && !matchMedia('(prefers-reduced-motion: reduce)').matches)
    frame = requestAnimationFrame(draw);
}

document.addEventListener('visibilitychange', () => animate(!document.hidden));
stop.addEventListener(
  'click',
  () => void (tauri ? invoke('voice_stop_speech') : Promise.resolve()),
);
mute.addEventListener('click', () => {
  const muted = state.mic !== 'muted';
  state.mic = muted ? 'muted' : 'listening';
  render();
  if (tauri) void invoke('voice_set_muted', { muted });
});

if (tauri) {
  void listen<readonly PillEvent[]>('alfa://events', (event) => {
    for (const e of event.payload) {
      if (e.type === 'VoicePill') Object.assign(state, e.state);
      else if (e.type === 'MicLevel') state.level = e.level;
    }
    render();
  });
}

render();
animate(!document.hidden);
