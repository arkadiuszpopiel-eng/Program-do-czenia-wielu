<!--
  Panel prawy — jedna karta naraz: Agentki · Oś czasu · Pliki · Pamięć · Ekran · Głos · Zadania
  (Alt+1…7). Zawartość kart ładowana leniwie; Ekran i Głos — puste stany kolejnych fal.
-->
<script lang="ts">
  import { Panel } from '@alfa/ui-kit';
  import type { PanelId } from '../../api/types-system';
  import { useApp } from '../../state/context';
  import Lazy from './Lazy.svelte';

  const app = useApp();
  const { t } = app.i18n;
  const TABS: readonly PanelId[] = [
    'agents',
    'timeline',
    'files',
    'memory',
    'tasks',
    'screen',
    'voice',
  ];
  const tabs = $derived(TABS.map((id) => ({ id, label: t(`panel.${id}`) })));
  const current = $derived(app.layout.current(app.activeId).right_tab);
  const WAVES: Readonly<Record<string, number>> = { screen: 6, voice: 2 };

  const loaders = {
    agents: () => import('../panels/AgentsPanel.svelte'),
    timeline: () => import('../panels/TimelinePanel.svelte'),
    files: () => import('../panels/FilesPanel.svelte'),
    memory: () => import('../panels/MemoryPanel.svelte'),
    tasks: () => import('../panels/TasksPanel.svelte'),
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
  {:else if current === 'memory'}
    <Lazy load={loaders.memory} props={{ sessionId: app.activeId }} label={t('common.loading')} />
  {:else if current === 'tasks'}
    <Lazy load={loaders.tasks} props={{ sessionId: app.activeId }} label={t('common.loading')} />
  {:else if current === 'screen' || current === 'voice'}
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
