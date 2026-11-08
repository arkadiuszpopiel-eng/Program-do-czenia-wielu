<!--
  Makieta ekranu F1 na PRAWDZIWYCH komponentach aplikacji: AppState + FakeAlfaClient w danym
  scenariuszu, motyw i stan startowy (widok, karta panelu, strona ustawień, krok wprowadzenia).
  Ramka 1280 × 800 px (szerokość laptopa, PLAN §14.10).
-->
<script lang="ts">
  import { untrack } from 'svelte';
  import App from '../App.svelte';
  import { FakeAlfaClient, type FakeScenario } from '../lib/api/fake/fake-client';
  import type { PanelId } from '../lib/api/types-system';
  import { AppState } from '../lib/state/app.svelte';

  interface Props {
    scenario?: FakeScenario;
    theme?: 'light' | 'dark';
    locale?: 'pl' | 'en';
    width?: number;
    height?: number;
    view?: 'chat' | 'settings' | 'onboarding';
    settingsPage?: string;
    hubWizard?: boolean;
    rightTab?: PanelId | null;
    leftOpen?: boolean;
    session?: string;
    onboardingStep?: number;
    palette?: boolean;
    cheatsheet?: boolean;
  }

  const props: Props = $props();
  const p = untrack(() => ({ ...props }));
  const client = new FakeAlfaClient({ scenario: p.scenario ?? 'default' });
  const app = new AppState(client);
  let ready = $state(false);

  void (async () => {
    await app.start();
    app.settings['ui.theme'] = p.theme ?? 'light';
    if (p.locale) await app.setSetting('ui.locale', p.locale);
    if (p.session) await app.openSession(p.session);
    const sid = app.activeId;
    if (p.leftOpen !== undefined) app.layout.update(sid, { left_open: p.leftOpen });
    if (p.rightTab) app.layout.update(sid, { right_open: true, right_tab: p.rightTab });
    if (p.onboardingStep !== undefined) app.onboardingStep = p.onboardingStep;
    if (p.view === 'settings') {
      app.hubWizard = p.hubWizard ?? false;
      app.openSettings(p.settingsPage);
    } else if (p.view) app.view = p.view;
    app.palette.open = p.palette ?? false;
    app.cheatsheetOpen = p.cheatsheet ?? false;
    ready = true;
  })();

  $effect(() => () => client.dispose());
</script>

<div class="frame" style:width="{p.width ?? 1280}px" style:height="{p.height ?? 800}px">
  {#if ready}<App {app} />{/if}
</div>

<style>
  .frame {
    position: relative;
    overflow: hidden;
    transform: translateZ(0);
    background: var(--alfa-color-bg);
  }
</style>
