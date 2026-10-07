<!--
  „Zawsze zezwalaj na podgląd pulpitu" dla agentki w sesji: intencja → okno Brokera
  (`gui.control(desktop.exe)` w zakresie ≤ 24 h). WebView niczego nie zatwierdza sam.
-->
<script lang="ts">
  import { Button, Select } from '@alfa/ui-kit';
  import { agentName } from '../../logic/work';
  import { useApp } from '../../state/context';

  interface Props {
    sessionId: string;
  }

  let { sessionId }: Props = $props();
  const app = useApp();
  const { t } = app.i18n;
  const cast = $derived((app.agents[sessionId] ?? []).map((a) => a.id as string));
  const options = $derived(
    (cast.length ? cast : ['delta']).map((id) => ({ value: id, label: agentName(id) })),
  );
  let agent = $state('delta');
  let busy = $state(false);

  $effect(() => {
    if (!options.some((o) => o.value === agent)) agent = options[0]?.value ?? 'delta';
  });

  async function request() {
    busy = true;
    try {
      const result = await app.client.gui.desktopGrant(sessionId, agent);
      app.toasts.show({
        kind: result.status === 'applied' ? 'success' : 'info',
        message:
          result.status === 'applied'
            ? t('gui.grantApplied', { name: agentName(agent) })
            : t('gui.grantBroker'),
      });
    } catch (error) {
      app.toasts.show({
        kind: 'error',
        message: error instanceof Error ? error.message : String(error),
      });
    } finally {
      busy = false;
    }
  }
</script>

<div class="grant">
  <p class="desc">{t('gui.grantDesc')}</p>
  <div class="row">
    <Select bind:value={agent} {options} label={t('gui.grantAgent')} />
    <Button size="sm" variant="secondary" disabled={busy} onclick={request}>{t('gui.grant')}</Button
    >
  </div>
</div>

<style>
  .grant {
    display: flex;
    flex-direction: column;
    gap: var(--alfa-space-2);
  }
  .row {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--alfa-space-2);
  }
  .desc {
    color: var(--alfa-color-text-muted);
    font-size: var(--alfa-font-size-xs);
  }
</style>
