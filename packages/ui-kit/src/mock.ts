// Atrapy danych dla makiet (Storybook i apps/desktop w F0). Nie są częścią API ui-kit.
import type { CaptionLine, ChatMessage, CommandItem, SessionItem } from './types';

export const mockSessions: readonly SessionItem[] = [
  { id: 's1', title: 'Raport Q3', project: 'Projekt X', active: true, working: true },
  { id: 's2', title: 'Kod: API płatności', project: 'Sklep' },
  { id: 's3', title: 'Zakupy na weekend', unread: true },
  { id: 's4', title: 'Plan wyjazdu do Gdańska' },
  { id: 's5', title: 'Migracja bazy', project: 'Sklep' },
];

export const mockMessages: readonly ChatMessage[] = [
  {
    id: 'm1',
    author: 'user',
    text: 'Przygotujcie raport Q3 dla zarządu: przychody, koszty i trzy rekomendacje. Dane są w folderze Finanse/2026-Q3.',
    time: '10:41',
  },
  {
    id: 'm2',
    author: 'alfa',
    role: 'Koordynatorka',
    text: 'Rozdzielam pracę: Gama zbierze dane z arkuszy, Delta przygotuje dokument, ja sprawdzę spójność liczb. Zacznę od struktury raportu.',
    time: '10:41',
  },
  {
    id: 'm3',
    author: 'gama',
    role: 'Analityczka',
    text: 'Zsumowałam przychody z trzech arkuszy: 4 812 300 zł (+11% r/r). Koszty operacyjne 3 106 900 zł. Marża wzrosła o 2,4 p.p. Szczegóły w tabeli poniżej.',
    time: '10:43',
    steps: [
      {
        id: 't1',
        icon: 'search',
        label: 'Przeszukano Finanse/2026-Q3 (14 plików)',
        durationMs: 820,
      },
      {
        id: 't2',
        icon: 'file',
        label: 'Odczytano przychody-Q3.xlsx, koszty-Q3.xlsx',
        durationMs: 1400,
      },
    ],
    variants: { index: 1, total: 2 },
  },
  {
    id: 'm4',
    author: 'delta',
    role: 'Wykonawczyni',
    text: 'Chcę utworzyć raport-Q3.docx w folderze Raporty i wstawić do niego tabelę Gamy oraz wykres marży.',
    time: '10:44',
    steps: [
      {
        id: 't3',
        icon: 'edit',
        label: 'Utworzono szkic raport-Q3.docx',
        durationMs: 2100,
        undoable: true,
      },
    ],
    approval: {
      what: 'Zapis pliku Raporty/raport-Q3.docx i nadpisanie szablonu zarząd.dotx',
      why: 'Zarząd używa tego szablonu; potrzebuję zaktualizować stopkę z kwartałem.',
      reversible: true,
      risk: 'medium',
    },
  },
  {
    id: 'm5',
    author: 'alfa',
    role: 'Koordynatorka',
    text: 'Sprawdzam sumy Gamy z księgą główną. Różnica 0,3% wynika z kursu EUR — dopiszę notę',
    time: '10:45',
    streaming: true,
  },
];

export const mockCommands: readonly CommandItem[] = [
  { id: 'new', label: 'Nowa rozmowa', group: 'Sesje', shortcut: 'Ctrl+N' },
  { id: 'switch', label: 'Przełącz sesję…', group: 'Sesje', shortcut: 'Ctrl+P' },
  { id: 'search', label: 'Szukaj wszędzie…', group: 'Sesje', shortcut: 'Ctrl+Shift+F' },
  { id: 'cast', label: 'Zmień obsadę agentek', group: 'Agentki', keywords: ['rola', 'obsada'] },
  { id: 'model', label: 'Zmień profil modelu', group: 'Agentki' },
  { id: 'mic', label: 'Mikrofon wł./wył.', group: 'Głos', shortcut: 'Ctrl+Shift+M' },
  { id: 'voice', label: 'Pełny tryb głosowy', group: 'Głos' },
  { id: 'focus', label: 'Tryb skupienia', group: 'Widok', shortcut: 'F11' },
  { id: 'left', label: 'Panel Sesje', group: 'Widok', shortcut: 'Ctrl+B' },
  { id: 'right', label: 'Panel prawy', group: 'Widok', shortcut: 'Ctrl+\\' },
  {
    id: 'theme',
    label: 'Przełącz motyw jasny/ciemny',
    group: 'Widok',
    keywords: ['dark', 'ciemny'],
  },
  { id: 'settings', label: 'Ustawienia', group: 'System', shortcut: 'Ctrl+,' },
  {
    id: 'stop',
    label: 'STOP WSZYSTKIEGO',
    group: 'System',
    shortcut: 'Ctrl+Shift+F12',
    keywords: ['kill'],
  },
];

export const mockCaptions: readonly CaptionLine[] = [
  {
    speaker: 'user',
    text: 'Przeczytaj mi podsumowanie raportu i powiedz, co jest najważniejsze',
    spokenChars: 43,
    partial: true,
  },
  {
    speaker: 'beta',
    text: 'Przychody w trzecim kwartale wzrosły o jedenaście procent rok do roku, głównie dzięki nowym klientom w segmencie B2B. Koszty utrzymały się na poziomie planu, a marża poprawiła się o dwa i cztery dziesiąte punktu.',
    spokenChars: 96,
    interruptedAt: 118,
  },
];

export const startSuggestions = [
  {
    title: 'Podsumuj folder',
    text: 'Przejrzyj Dokumenty/Umowy i wypisz, które kończą się w tym kwartale.',
  },
  { title: 'Zaplanuj tydzień', text: 'Ułóż plan na przyszły tydzień z kalendarza i listy zadań.' },
  {
    title: 'Napisz kod',
    text: 'Dodaj do projektu API endpoint eksportu zamówień do CSV z testami.',
  },
] as const;
