import {L,toggle} from './settings-helpers';
export const CONTINUATION_SETTINGS=[
  toggle('agents.auto_extend_steps',L('Zgoda na automatyczne przedłużanie kroków','Allow automatic step extensions'),L('Postęp jest zapisywany lokalnie. Wyłączone: zadanie czeka na Twoją zgodę na kolejną pulę. Limit kosztu nadal obowiązuje.','Progress is saved locally. Off: the task asks for another allowance. Cost limits still apply.'),false),
  toggle('agents.auto_extend_time',L('Zgoda na automatyczne przedłużanie czasu','Allow automatic active-time extensions'),L('Dotyczy czasu aktywnej pracy; oczekiwanie na zgodę nie powinno go zużywać.','Applies to active work time; approval waiting does not consume it.'),false),
  toggle('agents.auto_extend_approval',L('Automatycznie przedłużaj oczekiwanie na zatwierdzenie','Automatically extend approval waiting'),L('Nie zatwierdza działania za Ciebie. Jawna odmowa i anulowanie nadal zatrzymują pracę.','Does not approve actions on your behalf. Explicit denial and cancellation still stop execution.'),false),
];
const AFTER:Record<string,string>={'agents.max_steps':'agents.auto_extend_steps','agents.max_minutes':'agents.auto_extend_time','agents.approval_timeout_s':'agents.auto_extend_approval'};
export function withContinuation<T extends {key:string}>(defs:readonly T[]):(T|typeof CONTINUATION_SETTINGS[number])[]{return defs.flatMap((d)=>{const extra=CONTINUATION_SETTINGS.find((s)=>s.key===AFTER[d.key]);return extra?[d,extra]:[d];});}
