<!--
  „Zdjęcie z kamery” w polu pisania: dołącza do wiadomości jedną klatkę jako PNG — wtedy można
  zapytać „opisz mi to, co widzisz”. Widoczny tylko, gdy kamery są włączone i jest którą otworzyć;
  przy kilku kamerach najpierw wybór kamery. Zdjęcie jest zwykłym załącznikiem sesji.
-->
<script lang="ts">
  import { IconButton, Menu, type MenuItem } from '@alfa/ui-kit';
  import Camera from '@lucide/svelte/icons/camera';
  import type { AttachmentsState } from '../../state/attachments.svelte';
  import { useApp } from '../../state/context';

  interface Props {
    att: AttachmentsState;
  }
  let { att }: Props = $props();

  const app = useApp();
  const { t } = app.i18n;
  const usable = $derived(
    app.bool('camera.enabled', false)
      ? (app.cameras?.cameras ?? []).filter((c) => c.enabled && c.state !== 'unplugged')
      : [],
  );
  const items = $derived<MenuItem[]>(
    usable.map((c) => ({ id: c.id, label: c.name, onSelect: () => void att.camera(c.id) })),
  );
  const only = $derived(usable.length === 1 ? usable[0] : undefined);
</script>

{#if only}
  <IconButton
    label={t('cam.snap', { name: only.name })}
    size="sm"
    onclick={() => void att.camera(only.id)}
  >
    <Camera size={16} strokeWidth={1.5} />
  </IconButton>
{:else if usable.length > 1}
  <Menu {items} label={t('cam.snapPick')} align="end">
    {#snippet trigger(props)}
      <IconButton label={t('cam.snapPick')} size="sm" {...props}>
        <Camera size={16} strokeWidth={1.5} />
      </IconButton>
    {/snippet}
  </Menu>
{/if}
