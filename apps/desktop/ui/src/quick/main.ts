// Punkt wejścia okna Szybkiego pytania (osobne okno Tauri, wspólne środowisko WebView2).
import { mount } from 'svelte';
import '@alfa/ui-kit/tokens.css';
import '@alfa/ui-kit/base.css';
import './quick.css';
import QuickApp from './QuickApp.svelte';
import { createClient } from '../lib/api';

const target = document.getElementById('app');
if (!target) throw new Error('Brak elementu #app');

const client = await createClient();
const boot = await client.app.bootstrap();
document.documentElement.lang = boot.locale;
mount(QuickApp, { target, props: { client, locale: boot.locale } });
