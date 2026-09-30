<!-- Panel Sesje zwinięty do paska ikon (48 px). -->
<script lang="ts">
  import { IconButton } from '@alfa/ui-kit';
  import PanelLeftOpen from '@lucide/svelte/icons/panel-left-open';
  import Plus from '@lucide/svelte/icons/plus';
  import Search from '@lucide/svelte/icons/search';
  import { useApp } from '../../state/context';
  import { runCommand } from '../../state/commands';

  const app = useApp();
  const { t } = app.i18n;
</script>

<nav class="rail" aria-label={t('sessions.title')}>
  <IconButton label={t('sessions.expand')} onclick={() => app.layout.setLeftCollapsed(false)}>
    <PanelLeftOpen size={18} strokeWidth={1.5} />
  </IconButton>
  <IconButton label={t('sessions.new')} onclick={() => void app.newSession()}>
    <Plus size={18} strokeWidth={1.5} />
  </IconButton>
  <IconButton label={t('sessions.search')} onclick={() => runCommand(app, 'search.everywhere')}>
    <Search size={18} strokeWidth={1.5} />
  </IconButton>
</nav>

<style>
  .rail {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: var(--alfa-space-1);
    width: var(--alfa-size-sidebar-rail);
    height: 100%;
    padding: var(--alfa-space-2) 0;
    border-right: 1px solid var(--alfa-color-border);
    background: var(--alfa-color-surface);
  }
</style>
