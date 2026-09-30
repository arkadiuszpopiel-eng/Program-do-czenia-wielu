import { defineConfig } from 'vite';
import { svelte } from '@sveltejs/vite-plugin-svelte';

// Używane przez Storybook (@storybook/svelte-vite) — pakiet sam w sobie nie ma bundla:
// konsumenci importują źródła .svelte/.ts i budują je własnym Vite.
export default defineConfig({
  plugins: [svelte()],
});
