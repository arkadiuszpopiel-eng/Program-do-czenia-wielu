// Koszt tury do wiersza „szczegóły pod odpowiedzią” (zgłoszenie właściciela 2026-10-20: zawsze
// „0,00 zł”): model bez ceny w cenniku — „koszt nieznany”; ułamek grosza (tanie modele) — „< 0,01 zł”.
import type { Money, TurnUsage } from '../api/types';

export function costText(
  usage: TurnUsage,
  money: (value: Money) => string,
  unknown: string,
): string {
  if (usage.cost_micro_pln === null) return unknown;
  if (usage.cost.minor === 0 && usage.cost_micro_pln > 0) {
    return `< ${money({ minor: 1, currency: usage.cost.currency })}`;
  }
  return money(usage.cost);
}
