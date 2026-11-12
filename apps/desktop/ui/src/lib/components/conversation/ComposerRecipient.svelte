<script lang="ts">
  import { Chip, Menu, type MenuItem, type PersonaId } from '@alfa/ui-kit';
  import { useApp } from '../../state/context';

  interface Props {
    choices: readonly { id: PersonaId; name: string }[];
    chosen?: PersonaId | null;
  }
  let { choices, chosen = $bindable(null) }: Props = $props();
  const { t } = useApp().i18n;
  const name = $derived(choices.find((a) => a.id === chosen)?.name ?? t('composer.agentAuto'));
  const label = $derived(t('composer.agent', { name }));
  const items = $derived<MenuItem[]>([
    {
      id: 'auto',
      label: t('composer.agentAutoHint'),
      checked: chosen === null,
      onSelect: () => (chosen = null),
    },
    ...choices.map((a) => ({
      id: a.id,
      label: a.name,
      checked: chosen === a.id,
      onSelect: () => (chosen = a.id),
    })),
  ]);
</script>

<Menu {items} {label} align="end">
  {#snippet trigger(props)}
    <Chip {...props} agent={chosen ?? undefined} size="sm" onclick={props.onclick} {label}>
      <span class="recipient-name" title={name}>{name}</span><span aria-hidden="true">▾</span>
    </Chip>
  {/snippet}
</Menu>

<style>
  .recipient-name {
    max-width: 12ch;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
</style>
