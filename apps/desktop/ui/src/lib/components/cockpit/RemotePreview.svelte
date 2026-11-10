<!-- Podgląd układu bez transportu, parowania, QR i wysyłania poleceń. -->
<script lang="ts">
  import { Dialog } from 'bits-ui';
  import { Button } from '@alfa/ui-kit';
  import LockKeyhole from '@lucide/svelte/icons/lock-keyhole';
  import MessagesSquare from '@lucide/svelte/icons/messages-square';
  import Mic from '@lucide/svelte/icons/mic';
  import ListChecks from '@lucide/svelte/icons/list-checks';
  import Folder from '@lucide/svelte/icons/folder';
  import Monitor from '@lucide/svelte/icons/monitor';
  import Users from '@lucide/svelte/icons/users';
  import Cpu from '@lucide/svelte/icons/cpu';
  import ShieldCheck from '@lucide/svelte/icons/shield-check';
  import { useApp } from '../../state/context';
  import { remoteCopy } from './remote-copy';
  import './remote-preview.css';
  let { onclose }: { onclose: () => void } = $props();
  const app = useApp();
  const uid = $props.id();
  const copy = $derived(remoteCopy(app.i18n.locale));
  const actions = $derived([
    { id: 'chat', label: copy.chat, icon: MessagesSquare },
    { id: 'voice', label: copy.voice, icon: Mic },
    { id: 'tasks', label: copy.tasks, icon: ListChecks },
    { id: 'files', label: copy.files, icon: Folder },
    { id: 'screen', label: copy.screen, icon: Monitor },
    { id: 'agents', label: copy.agents, icon: Users },
    { id: 'models', label: copy.models, icon: Cpu },
    { id: 'access', label: copy.access, icon: ShieldCheck },
  ]);
</script>

<Dialog.Root open onOpenChange={(open) => !open && onclose()}>
  <Dialog.Portal>
    <Dialog.Overlay class="phone-overlay" />
    <Dialog.Content
      class="phone-panel"
      data-connection="unconfigured"
      onOpenAutoFocus={(event) => {
        event.preventDefault();
        document.getElementById(`${uid}-title`)?.focus();
      }}
    >
      <p class="phone-eyebrow">ALFA / REMOTE</p>
      <Dialog.Title class="phone-title" id="{uid}-title" tabindex={-1}>{copy.title}</Dialog.Title>
      <Dialog.Description class="phone-lead">{copy.lead}</Dialog.Description>
      <p class="phone-status" role="status">
        <LockKeyhole size={18} aria-hidden="true" />{copy.status}
      </p>
      <p class="phone-reason" id="{uid}-reason">{copy.reason}</p>
      <Button variant="secondary" disabled aria-describedby="{uid}-reason">{copy.pairing}</Button>
      <section class="phone-actions" aria-label={copy.actions}>
        {#each actions as action (action.id)}
          <button
            type="button"
            data-remote-action={action.id}
            disabled
            aria-describedby="{uid}-reason"
          >
            <action.icon size={24} strokeWidth={1.5} aria-hidden="true" /><span>{action.label}</span
            >
          </button>
        {/each}
      </section>
      <div class="phone-footer">
        <Button variant="primary" onclick={onclose}>{copy.close}</Button>
      </div>
    </Dialog.Content>
  </Dialog.Portal>
</Dialog.Root>
