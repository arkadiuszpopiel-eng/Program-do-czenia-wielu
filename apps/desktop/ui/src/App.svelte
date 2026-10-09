<!-- Korzeń okna głównego: kontekst stanu, wygląd, skróty, widok (start / wprowadzenie / powłoka). -->
<script lang="ts">
  import { untrack } from 'svelte';
  import { Button, CONTEXT_MENU_LABELS, installContextMenu, installTooltips } from '@alfa/ui-kit';
  import { applyAppearance, effectiveWidth } from './lib/appearance';
  import AppShell from './lib/components/shell/AppShell.svelte';
  import Lazy from './lib/components/shell/Lazy.svelte';
  import type { AppState } from './lib/state/app.svelte';
  import { handleKeydown } from './lib/state/commands';
  import { provideApp } from './lib/state/context';

  interface Props {
    app: AppState;
  }

  const props: Props = $props();
  // Instancja stanu jest stała przez całe życie okna — odczyt raz, poza śledzeniem.
  const app = untrack(() => props.app);
  provideApp(app);
  const { t } = app.i18n;
  const loadOnboarding = () => import('./lib/views/onboarding/OnboardingView.svelte');
  // Terminal (gest użytkownika): emulator ładowany leniwie, poza paczką startową.
  const loadTerminal = () => import('./lib/components/overlays/TerminalDialog.svelte');
  // „Co nowego" raz po aktualizacji — ładowane leniwie tylko wtedy.
  const loadWhatsNew = () => import('./lib/components/overlays/WhatsNewDialog.svelte');

  // Szerokość okna steruje skalą dopasowania (`ui.zoom_fit`) i trybem układu.
  let windowWidth = $state(window.innerWidth);

  $effect(() => {
    applyAppearance(document.documentElement, {
      theme: app.str('ui.theme', 'auto'),
      style: app.str('ui.style', 'classic'),
      density: app.str('ui.density', 'comfortable'),
      animations: app.bool('ui.animations', true),
      zoom: app.num('ui.zoom', 100),
      fit: app.bool('ui.zoom_fit', true),
      width: windowWidth,
    });
  });

  $effect(() => {
    const zoom = app.num('ui.zoom', 100);
    const fit = app.bool('ui.zoom_fit', true);
    const update = () => {
      windowWidth = window.innerWidth;
      app.layout.width = effectiveWidth(window, zoom, fit);
    };
    update();
    window.addEventListener('resize', update);
    return () => window.removeEventListener('resize', update);
  });

  $effect(() => () => app.dispose());

  // Menu kontekstowe i podpowiedzi rysuje Alfa — systemowe menu WebView i dymki `title` nie
  // pojawiają się nigdzie.
  $effect(() => {
    const menu = installContextMenu({
      labels: () => CONTEXT_MENU_LABELS[app.i18n.locale],
      readClipboard: () => app.client.system.clipboardText(),
      notify: (message) => app.toasts.show({ kind: 'warning', message }),
    });
    const tips = installTooltips();
    return () => {
      menu();
      tips();
    };
  });
</script>

<svelte:window onkeydown={(e) => handleKeydown(app, e)} />

{#if app.view === 'loading'}
  <div class="boot" data-surface="app" role="status" aria-label={t('app.loading')}></div>
{:else if app.view === 'error'}
  <!-- Start nieudany (np. rdzeń nie odpowiedział): komunikat i „Ponów" zamiast martwego okna. -->
  <div class="fatal">
    <p role="alert">{t('app.failed', { error: app.fatal ?? '' })}</p>
    <Button variant="secondary" onclick={() => void app.start()}>{t('common.retry')}</Button>
  </div>
{:else if app.view === 'onboarding'}
  <Lazy load={loadOnboarding} props={{}} label={t('app.loading')} />
{:else}
  <AppShell />
{/if}
{#if app.work.terminal}
  <Lazy load={loadTerminal} props={{ request: app.work.terminal }} label={t('app.loading')} />
{/if}
{#if app.updates.whatsNew && (app.view === 'chat' || app.view === 'settings')}
  <Lazy load={loadWhatsNew} props={{ news: app.updates.whatsNew }} label={t('app.loading')} />
{/if}

<style>
  .boot {
    height: 100%;
    background: var(--alfa-color-bg);
  }
  .fatal {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: var(--alfa-space-3);
    margin: var(--alfa-space-8);
  }
  .fatal p {
    margin: 0;
    color: var(--alfa-color-error);
  }
</style>
