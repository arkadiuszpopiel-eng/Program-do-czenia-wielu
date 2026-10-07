<script lang="ts">
  import { Toast } from '@alfa/ui-kit';
  import { useApp } from '../../state/context';

  const app = useApp();
  const { t } = app.i18n;
</script>

<div class="toasts" role="region" aria-label={t('toast.region')}>
  {#each app.toasts.items as toast (toast.id)}
    <Toast
      kind={toast.kind}
      message={toast.message}
      actionLabel={toast.actionLabel}
      onaction={toast.onAction ? () => app.toasts.act(toast.id) : undefined}
      onclose={() => app.toasts.dismiss(toast.id)}
      closeLabel={t('toast.close')}
      onpause={() => app.toasts.pause(toast.id)}
      onresume={() => app.toasts.resume(toast.id)}
    />
  {/each}
</div>

<style>
  .toasts {
    position: fixed;
    right: var(--alfa-space-4);
    bottom: var(--alfa-space-4);
    z-index: 90;
    display: flex;
    flex-direction: column;
    align-items: flex-end;
    gap: var(--alfa-space-2);
    pointer-events: none;
  }
  .toasts > :global(*) {
    pointer-events: auto;
  }
</style>
