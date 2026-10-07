// Punkt wejścia okna Szybkiego pytania (osobne okno Tauri, wspólne środowisko WebView2).
import { mount } from 'svelte';
import { installDefaultPolicy } from '@alfa/ui-kit';
import '@alfa/ui-kit/tokens.css';
import '@alfa/ui-kit/base.css';
import './quick.css';
import QuickApp from './QuickApp.svelte';
import { createClient } from '../lib/api';
import { errorText } from '../lib/api/command-error';
import { applyBootDocument } from '../lib/window-boot';
import { quickText } from './strings';

// Trusted Types: polityka `default` tylko dla workera podświetlania (CSP wydania, PT-33).
installDefaultPolicy();

const target = document.getElementById('app');
if (!target) throw new Error('Brak elementu #app');

const client = await createClient();
// Język i motyw (`ui.theme`) z ustawień — przed montażem, żeby pierwsza klatka była już właściwa.
// Błąd odczytu nie zostawia pustego okna: polski i motyw systemu (jak pigułka); pytanie i tak
// pokaże błąd rdzenia przy polu.
const boot = await client.app.bootstrap().catch((error: unknown) => {
  console.warn(`Alfa: app_bootstrap: ${errorText(error)}`);
  return null;
});
const locale = boot?.locale ?? 'pl';
applyBootDocument(document.documentElement, { locale, settings: boot?.settings ?? {} });
document.title = `Alfa — ${quickText(locale, 'title')}`;
mount(QuickApp, { target, props: { client, locale } });
