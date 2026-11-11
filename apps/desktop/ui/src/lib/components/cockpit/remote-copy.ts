const pl = {
  title: 'Panel telefonu — podgląd',
  lead: 'Przegląd układu sterowania. Telefon nie jest połączony z komputerem.',
  status: 'Brak połączenia · sterowanie zablokowane',
  pairing: 'Sparuj urządzenie',
  reason:
    'Parowanie nie jest jeszcze dostępne. Polecenia pozostają nieaktywne do zestawienia uwierzytelnionego połączenia.',
  actions: 'Zakres panelu zdalnego',
  chat: 'Rozmowa',
  voice: 'Głos',
  tasks: 'Zadania',
  files: 'Pliki',
  screen: 'Ekran',
  agents: 'Agentki',
  models: 'Modele',
  access: 'Uprawnienia',
  close: 'Wróć do kokpitu',
};
const en: typeof pl = {
  title: 'Phone panel — preview',
  lead: 'Preview of the control layout. Your phone is not connected to the computer.',
  status: 'Not connected · controls locked',
  pairing: 'Pair device',
  reason:
    'Pairing is not available yet. Commands remain disabled until an authenticated connection is established.',
  actions: 'Remote panel scope',
  chat: 'Conversation',
  voice: 'Voice',
  tasks: 'Tasks',
  files: 'Files',
  screen: 'Screen',
  agents: 'Agents',
  models: 'Models',
  access: 'Permissions',
  close: 'Back to cockpit',
};
export const remoteCopy = (locale: string) => (locale === 'en' ? en : pl);
