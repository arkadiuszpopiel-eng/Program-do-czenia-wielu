// Atrapa: Ustawienia → Zaawansowane. Dwa pliki konfiguracji w pamięci; prosta kontrola składni
// (nawias tabeli, `klucz = wartość`) i reguły rdzenia: tabela `[kernel]` odrzucona (polityka Jądra
// tylko przez Brokera), poprzednia treść zostaje.
import type { AdvancedApi, RawLayer } from '../client-advanced';
import type { RawConfig } from '../types-advanced';
import type { FakeCore } from './core';

const SHARED = '[ui]\ntheme = "auto"\nscale = 1.0\n\n[voice]\nmic_enabled = true\n';
const MACHINE = '[camera]\nenabled = false\n';
const TABLE = /^\[[A-Za-z0-9_.-]+\]$/;
const PAIR = /^[A-Za-z0-9_.-]+\s*=\s*\S/;

/** Liczba kluczy albo błąd z numerem wiersza (uproszczony TOML). */
export function fakeCheck(text: string): number {
  let keys = 0;
  text.split(/\r?\n/).forEach((raw, i) => {
    const line = raw.trim();
    if (!line || line.startsWith('#')) return;
    const ok = line.startsWith('[') ? TABLE.test(line) : PAIR.test(line);
    if (!ok) throw new Error(`Błąd składni TOML: wiersz ${i + 1}.`);
    if (!line.startsWith('[')) keys += 1;
  });
  return keys;
}

export class FakeAdvanced {
  private files: Record<RawLayer, string> = { shared: SHARED, machine: MACHINE };

  constructor(private readonly core: FakeCore) {}

  private view(): RawConfig {
    return {
      shared: this.files.shared,
      machine: this.files.machine,
      shared_path: 'C:\\Users\\Ty\\AppData\\Roaming\\Alfa\\config\\shared.toml',
      machine_path: 'C:\\Users\\Ty\\AppData\\Roaming\\Alfa\\config\\machine\\laptop.toml',
      problems: [],
    };
  }

  private save(layer: RawLayer, text: string): RawConfig {
    fakeCheck(text);
    if (/^\s*\[kernel/m.test(text)) {
      throw new Error(
        'Konfiguracja odrzucona, plik przywrócony: plik zmienia politykę Jądra z pominięciem Sędziego Cieni.',
      );
    }
    this.files = { ...this.files, [layer]: text };
    return this.view();
  }

  api(): AdvancedApi {
    return {
      raw: () => this.core.reply(this.view()),
      check: async (text) => this.core.reply(fakeCheck(text)),
      save: async (layer, text) => this.core.reply(this.save(layer, text)),
    };
  }
}
