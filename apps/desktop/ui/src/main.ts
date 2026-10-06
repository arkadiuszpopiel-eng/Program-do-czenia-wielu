import { mount } from 'svelte';
import { installDefaultPolicy } from '@alfa/ui-kit';
import '@alfa/ui-kit/tokens.css';
import '@alfa/ui-kit/base.css';
import './app.css';
import App from './App.svelte';
import { createClient } from './lib/api';
import { AppState } from './lib/state/app.svelte';

// Trusted Types: polityka `default` tylko dla workera podświetlania (CSP wydania, PT-33).
installDefaultPolicy();

const target = document.getElementById('app');
if (!target) throw new Error('Brak elementu #app');

const client = await createClient();
const app = new AppState(client);
mount(App, { target, props: { app } });
void app.start();
