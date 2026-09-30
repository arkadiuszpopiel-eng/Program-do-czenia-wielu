<!--
  Panel prawy — jedna karta naraz: Agentki · Oś czasu · Pliki · Pamięć · Ekran · Głos (Alt+1…6).
  Zawartość kart ładowana leniwie; w F1 działają Agentki, Oś czasu v0 i Pliki.
-->
<script lang="ts">
  import { Panel } from '@alfa/ui-kit';
  import type { PanelId } from '../../api/types-system';
  import { useApp } from '../../state/context';
  import Lazy from './Lazy.svelte';

  const app = useApp();
  const { t } = app.i18n;
  const TABS: readonly PanelId[] = ['agents', 'timeline', 'files', 'memory', 'screen', 'voice'];
  const tabs = $derived(TABS.map((id) => ({ id, label: t(`panel.${id}`) })));
  const current = $derived(app.layout.current(app.activeId).right_tab);
  const WAVES: Readonly<Record<string, number>> = { memory: 7, screen: 6, voice: 2 };

  const loaders = {
    agents: () => import('../panels/AgentsPanel.svelte'),
    timeline: () => import('../panels/TimelinePanel.svelte'),
    files: () => import('../panels/FilesPanel.svelte'),
  };
</script>

<Panel
  title={t('panel.label')}
  {tabs}
  bind:activeTab={() => current, (tab) => app.openPanel(tab as PanelId)}
  onclose={() => app.layout.update(app.activeId, { right_open: false })}
  closeLabel={t('panel.close')}
  tabsLabel={t('panel.label')}
  hideHeader
>
  {#if !app.activeId}
    <p class="muted">{t('sessions.empty')}</p>
  {:else if current === 'agents'}
    <Lazy load={loaders.agents} props={{ sessionId: app.activeId }} label={t('common.loading')} />
  {:else if current === 'timeline'}
    <Lazy load={loaders.timeline} props={{ sessionId: app.activeId }} label={t('common.loading')} />
  {:else if current === 'files'}
    <Lazy load={loaders.files} props={{ sessionId: app.activeId }} label={t('common.loading')} />
  {:else}
    <div class="later">
      <h3>{t(`panel.${current}`)}</h3>
      <p>{app.i18n.tk(`panel.later.${current}`)}</p>
      <p class="muted">{t('panel.laterWave', { wave: WAVES[current] ?? 2 })}</p>
    </div>
  {/if}
</Panel>

<style>
  .later {
    display: flex;
    flex-direction: column;
    gap: var(--alfa-space-2);
    padding: var(--alfa-space-6) var(--alfa-space-2);
    text-align: center;
    font-size: var(--alfa-font-size-sm);
  }
  .muted {
    color: var(--alfa-color-text-muted);
    font-size: var(--alfa-font-size-sm);
  }
</style>
