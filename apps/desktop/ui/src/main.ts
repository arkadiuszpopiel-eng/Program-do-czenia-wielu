import { mount } from 'svelte';
import '@alfa/ui-kit/tokens.css';
import '@alfa/ui-kit/base.css';
import './app.css';
import App from './App.svelte';

const target = document.getElementById('app');
if (!target) throw new Error('Brak elementu #app');

mount(App, { target });
