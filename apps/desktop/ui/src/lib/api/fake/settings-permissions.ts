// Atrapa drzewa §15: ustawienia na stronie „Uprawnienia i bezpieczeństwo” — te same klucze i teksty
// co `crates/app-core/data/settings-pages.json` (ukrywanie okien na zrzutach, baner trybu
// deweloperskiego Brokera).
import type { SettingDef } from '../types-system';
import { L, toggle } from './settings-helpers';

export const PERMISSIONS_SETTINGS: SettingDef[] = [
  toggle(
    'ui.hide_from_capture',
    L(
      'Ukrywaj okna Alfy na zrzutach i nagraniach ekranu',
      'Hide Alfa windows from screenshots and screen recordings',
    ),
    L(
      'Włączone: Windows wycina okna Alfy ze zrzutów (Wycinek, PrintScreen) i nagrań innych programów — w ich miejscu jest czarne pole albo pulpit. Wyłączone (domyślnie): zrzuty działają normalnie, możesz pokazać ekran przy zgłaszaniu błędu. Agentki i tak nie widzą okien Alfy ani okna zatwierdzeń — ich zrzuty maskują procesy Alfy niezależnie od tego ustawienia. Zalecenie: wyłączone; włącz przy udostępnianiu ekranu, gdy w rozmowie są dane prywatne. Zmiana działa po ponownym uruchomieniu Alfy.',
      "On: Windows cuts Alfa windows out of screenshots (Snipping Tool, PrintScreen) and other programs' recordings — a black area or the desktop shows instead. Off (default): screenshots work normally, so you can show the screen when reporting a problem. Agents still cannot see Alfa windows or the approval window — their screenshots mask Alfa processes regardless of this setting. Recommended: off; turn on when sharing your screen while the conversation holds private data. Takes effect after restarting Alfa.",
    ),
    false,
    'machine',
  ),
  toggle(
    'ui.broker_dev_banner',
    L('Pokazuj baner trybu deweloperskiego', 'Show the developer mode banner'),
    L(
      'Baner „Tryb deweloperski: Sędzia Cieni w procesie aplikacji” to tylko informacja. Wyłącz, żeby nie pokazywał się przy każdym uruchomieniu — tryb pracy się nie zmienia. Banery bezpiecznego stanu (brak Sędziego Cieni, zerwane połączenie) pokazują się zawsze.',
      'The “Developer mode: Shadow Judge inside the app process” banner is information only. Turn off so it no longer shows on every start — the mode itself does not change. Safe-state banners (no Shadow Judge, lost connection) always show.',
    ),
    true,
  ),
];
