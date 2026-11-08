// Pomocnicze konstruktory definicji ustawień (atrapa drzewa §15).
import type { LocalizedText } from '../types';
import type { SettingDef, SettingScope, SettingsPageDef } from '../types-system';

export const L = (pl: string, en: string): LocalizedText => ({ pl, en });

export const toggle = (
  key: string,
  label: LocalizedText,
  description: LocalizedText,
  def: boolean,
  scope: SettingScope = 'global',
): SettingDef => ({
  key,
  label,
  description,
  control: { kind: 'toggle' },
  default: def,
  scope,
});

export const select = (
  key: string,
  label: LocalizedText,
  description: LocalizedText,
  options: [string, LocalizedText][],
  def: string,
  scope: SettingScope = 'global',
): SettingDef => ({
  key,
  label,
  description,
  default: def,
  scope,
  control: { kind: 'select', options: options.map(([value, text]) => ({ value, label: text })) },
});

export const number = (
  key: string,
  label: LocalizedText,
  description: LocalizedText,
  def: number,
  min: number,
  max: number,
  step: number,
  unit: string | null,
  scope: SettingScope = 'global',
): SettingDef => ({
  key,
  label,
  description,
  default: def,
  scope,
  control: { kind: 'number', min, max, step, unit },
});

export const text = (
  key: string,
  label: LocalizedText,
  description: LocalizedText,
  def: string,
  scope: SettingScope = 'global',
): SettingDef => ({
  key,
  label,
  description,
  default: def,
  scope,
  control: { kind: 'text' },
});

export const page = (
  id: string,
  label: LocalizedText,
  wave: number,
  settings: SettingDef[],
  extra: Partial<SettingsPageDef> = {},
): SettingsPageDef => ({
  id,
  label,
  wave,
  custom: null,
  settings,
  upcoming: [],
  ...extra,
});

export const later = (
  id: string,
  label: LocalizedText,
  wave: number,
  upcoming: LocalizedText[],
): SettingsPageDef => page(id, label, wave, [], { upcoming });
