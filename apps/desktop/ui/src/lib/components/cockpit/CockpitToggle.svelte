<script lang="ts">
  import { IconButton } from '@alfa/ui-kit';
  import Radar from '@lucide/svelte/icons/radar';
  import { useApp } from '../../state/context';
  import { cockpitCopy } from './copy';
  const app = useApp();
  const copy = $derived(cockpitCopy(app.i18n.locale));
</script>

<IconButton
  label={app.layout.cockpit ? copy.off : copy.on}
  pressed={app.layout.cockpit}
  size="sm"
  onclick={() => app.layout.toggleCockpit()}
>
  <Radar size={18} strokeWidth={1.5} />
</IconButton>
