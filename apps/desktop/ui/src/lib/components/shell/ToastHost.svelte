<script lang="ts">
  import { Toast } from '@alfa/ui-kit';
  import { useApp } from '../../state/context';

  const app = useApp();
  const { t } = app.i18n;

  // Powiadomienia nad paskiem pisania — dwa naraz przykrywały żeton trybu pracy i przycisk
  // wysyłania (test menu trybu, 3.11). Liczone przy każdej zmianie listy powiadomień.
  let lift = $state(0);
  $effect(() => {
    void app.toasts.items.length;
    const rect = document.querySelector('form.composer')?.getBoundingClientRect();
    lift = rect && rect.height > 0 ? Math.max(0, Math.round(window.innerHeight - rect.top)) : 0;
  });
</script>

<div class="toasts" role="region" aria-label={t('toast.region')} style:--lift="{lift}px">
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
    bottom: calc(var(--lift, 0px) + var(--alfa-space-4));
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
