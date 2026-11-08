// Makiety ekranów F1 (PLAN §14.10) jako klikalne strony na prawdziwych komponentach aplikacji,
// z atrapą backendu. Każdy ekran w wariancie jasnym i ciemnym, szerokość 1280 px.
import type { Meta, StoryObj } from '@storybook/svelte-vite';
import type { ComponentProps } from 'svelte';
import StoryApp from './StoryApp.svelte';

type Args = ComponentProps<typeof StoryApp>;

const meta = {
  title: 'Ekrany F1',
  component: StoryApp,
  parameters: { layout: 'fullscreen', a11y: { test: 'error' } },
} satisfies Meta<typeof StoryApp>;
export default meta;

type Story = StoryObj<typeof meta>;

const light = (args: Args): Story => ({
  args: { ...args, theme: 'light' },
  globals: { theme: 'light' },
});
const dark = (args: Args): Story => ({
  args: { ...args, theme: 'dark' },
  globals: { theme: 'dark' },
});

/** 1 · Start / pusty stan (3 podpowiedzi). */
export const S01_Start_jasny = light({ scenario: 'empty' });
export const S01_Start_ciemny = dark({ scenario: 'empty' });
/** 2 · Rozmowa: warianty ‹ 1/2 ›, kroki narzędzi, blok kodu, karta „czeka na zatwierdzenie". */
export const S02_Rozmowa_jasny = light({});
export const S02_Rozmowa_ciemny = dark({});
/** 2 · Rozmowa z paletą poleceń (Ctrl+K). */
export const S02b_Paleta_jasny = light({ palette: true });
export const S02b_Paleta_ciemny = dark({ palette: true });
/** 2 · Ściągawka skrótów (Ctrl+/). */
export const S02c_Sciagawka_jasny = light({ cheatsheet: true });
export const S02c_Sciagawka_ciemny = dark({ cheatsheet: true });
/** 6 · Panel Agentki / obsada ról. */
export const S06_Agentki_jasny = light({ leftOpen: false, rightTab: 'agents' });
export const S06_Agentki_ciemny = dark({ leftOpen: false, rightTab: 'agents' });
/** 7 · Oś czasu v0. */
export const S07_OsCzasu_jasny = light({ leftOpen: false, rightTab: 'timeline' });
export const S07_OsCzasu_ciemny = dark({ leftOpen: false, rightTab: 'timeline' });
/** 8 · Artefakty i podgląd pliku. */
export const S08_Artefakty_jasny = light({ leftOpen: false, rightTab: 'files' });
export const S08_Artefakty_ciemny = dark({ leftOpen: false, rightTab: 'files' });
/** 11 · Ustawienia: drzewo + wyszukiwarka. */
export const S11_Ustawienia_jasny = light({ view: 'settings', settingsPage: 'general' });
export const S11_Ustawienia_ciemny = dark({ view: 'settings', settingsPage: 'general' });
/** 11 · Ustawienia › Wygląd. */
export const S11b_Wyglad_jasny = light({ view: 'settings', settingsPage: 'appearance' });
export const S11b_Wyglad_ciemny = dark({ view: 'settings', settingsPage: 'appearance' });
/** 11 · Ustawienia › Koszty (limit PLN z przełącznikiem wyłączenia). */
export const S11c_Koszty_jasny = light({ view: 'settings', settingsPage: 'costs' });
export const S11c_Koszty_ciemny = dark({ view: 'settings', settingsPage: 'costs' });
/** 11 · Ustawienia › Uprawnienia (L0–L4, zmiana przez Brokera). */
export const S11d_Uprawnienia_jasny = light({ view: 'settings', settingsPage: 'permissions' });
export const S11d_Uprawnienia_ciemny = dark({ view: 'settings', settingsPage: 'permissions' });
/** 11 · Ustawienia › Skróty (konflikty, reguła AltGr). */
export const S11e_Skroty_jasny = light({ view: 'settings', settingsPage: 'shortcuts' });
export const S11e_Skroty_ciemny = dark({ view: 'settings', settingsPage: 'shortcuts' });
/** 11 · Ustawienia › Urządzenia (profil sprzętu). */
export const S11f_Urzadzenia_jasny = light({ view: 'settings', settingsPage: 'devices' });
export const S11f_Urzadzenia_ciemny = dark({ view: 'settings', settingsPage: 'devices' });
/** 12 · Hub kont i kluczy. */
export const S12_Hub_jasny = light({ view: 'settings', settingsPage: 'providers' });
export const S12_Hub_ciemny = dark({ view: 'settings', settingsPage: 'providers' });
/** 12 · Kreator „Dodaj dostawcę" (brak kluczy). */
export const S12b_Kreator_jasny = light({
  scenario: 'no-keys',
  view: 'settings',
  settingsPage: 'providers',
  hubWizard: true,
});
export const S12b_Kreator_ciemny = dark({
  scenario: 'no-keys',
  view: 'settings',
  settingsPage: 'providers',
  hubWizard: true,
});
/** 13 · Import / eksport `.alfa`. */
export const S13_Transfer_jasny = light({ view: 'settings', settingsPage: 'transfer' });
export const S13_Transfer_ciemny = dark({ view: 'settings', settingsPage: 'transfer' });
/** 18 · Stany: offline (baner + kolejka). */
export const S18a_Offline_jasny = light({ scenario: 'offline' });
export const S18a_Offline_ciemny = dark({ scenario: 'offline' });
/** 18 · Stany: 429 (kiedy się odnowi). */
export const S18b_Limit429_jasny = light({ scenario: 'rate-limited' });
export const S18b_Limit429_ciemny = dark({ scenario: 'rate-limited' });
/** 18 · Stany: brak kluczy (profil lokalny). */
export const S18c_BrakKluczy_jasny = light({ scenario: 'no-keys' });
export const S18c_BrakKluczy_ciemny = dark({ scenario: 'no-keys' });
/** 18 · Stany: brak zgody na mikrofon. */
export const S18d_Mikrofon_jasny = light({ scenario: 'no-mic' });
export const S18d_Mikrofon_ciemny = dark({ scenario: 'no-mic' });
/** 18 · Stany: mało miejsca na dysku. */
export const S18e_Dysk_jasny = light({ scenario: 'disk-low' });
export const S18e_Dysk_ciemny = dark({ scenario: 'disk-low' });
/** Responsywność: 900 px (szuflady) i 600 px (tryb kompaktowy). */
export const R900_jasny = light({ width: 900, height: 700 });
export const R900_ciemny = dark({ width: 900, height: 700 });
export const R600_jasny = light({ width: 600, height: 760, leftOpen: false });
export const R600_ciemny = dark({ width: 600, height: 760, leftOpen: false });
/** Interfejs po angielsku (i18n od dnia 0). */
export const EN_jasny = light({ locale: 'en' });
export const EN_ciemny = dark({ locale: 'en' });
