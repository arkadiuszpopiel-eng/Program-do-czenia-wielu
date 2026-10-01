<!-- Węzeł drzewa zadań: stan, postęp, pochodzenie, zależności, sterowanie i podzadania. -->
<script lang="ts">
  import { Avatar, Button, TextField, type AgentId } from '@alfa/ui-kit';
  import type { TaskInfo } from '../../api/types-tasks';
  import { useApp } from '../../state/context';
  import TaskNode from './TaskNode.svelte';

  interface Props {
    task: TaskInfo;
    childrenOf: (id: string) => readonly TaskInfo[];
    titleOf: (id: string) => string;
  }

  let { task, childrenOf, titleOf }: Props = $props();
  const app = useApp();
  const { t, tk } = app.i18n;
  const AGENTS = ['alfa', 'beta', 'gama', 'delta'];
  let message = $state('');
  const done = $derived(task.state === 'done');
  const kids = $derived(childrenOf(task.id));
  const status = $derived(
    done && task.result ? tk(`tasks.result.${task.result}`) : tk(`tasks.state.${task.state}`),
  );

  async function run(action: () => Promise<unknown>, success?: string) {
    try {
      await action();
      if (success) app.toasts.show({ kind: 'success', message: success });
    } catch (error) {
      app.toasts.show({
        kind: 'error',
        message: error instanceof Error ? error.message : String(error),
      });
    }
  }

  async function steer() {
    const text = message.trim();
    if (!text) return;
    await run(() => app.client.tasks.steer(task.id, text), t('tasks.steered'));
    message = '';
  }
</script>

<li class="node" data-state={task.state} data-result={task.result ?? undefined}>
  <div class="head">
    {#if task.agent && AGENTS.includes(task.agent)}
      <Avatar agent={task.agent as AgentId} size={20} />
    {/if}
    <span class="title">{task.title}</span>
    <span class="status">{status}</span>
  </div>
  {#if !done && task.max_steps > 0}
    <progress
      max={task.max_steps}
      value={task.steps}
      aria-label={t('tasks.progress', { steps: task.steps, max: task.max_steps })}
    ></progress>
  {/if}
  <p class="meta">
    {tk(`tasks.origin.${task.origin}`)}{#if task.deps.length}
      · {t('tasks.depsOn', { list: task.deps.map((d) => titleOf(d.task_id)).join(', ') })}{/if}
    · {t('tasks.progress', { steps: task.steps, max: task.max_steps })}{#if task.attempt > 1}
      · {t('tasks.attempt', {
        n: task.attempt,
        max: task.max_attempts,
      })}{/if}{#if task.cost.minor > 0}
      · {t('tasks.cost', { cost: app.i18n.money(task.cost) })}{/if}
  </p>
  {#if task.executor.startsWith('bridge:')}<p class="note">{t('tasks.bridge')}</p>{/if}
  {#if task.tainted}<p class="note">{t('tasks.tainted')}</p>{/if}
  {#if task.blocked && !done}<p class="meta">{task.blocked}</p>{/if}
  {#if task.error}<p class="error" data-selectable>{task.error}</p>{/if}
  {#if task.summary}<p class="summary" data-selectable>{task.summary}</p>{/if}
  <div class="actions" role="group" aria-label={t('tasks.actions', { title: task.title })}>
    {#if done}
      <Button
        size="sm"
        variant="secondary"
        onclick={() => run(() => app.client.tasks.retry(task.id), t('tasks.retried'))}
        >{t('tasks.retry')}</Button
      >
    {:else}
      <form
        class="steer"
        onsubmit={(e) => {
          e.preventDefault();
          void steer();
        }}
      >
        <TextField label={t('tasks.steer')} bind:value={message} />
        <Button size="sm" variant="secondary" type="submit">{t('tasks.send')}</Button>
      </form>
      {#if task.state === 'paused'}
        <Button
          size="sm"
          variant="secondary"
          onclick={() => run(() => app.client.tasks.resume(task.id))}>{t('tasks.resume')}</Button
        >
      {:else}
        <Button
          size="sm"
          variant="secondary"
          onclick={() => run(() => app.client.tasks.pause(task.id))}>{t('tasks.pause')}</Button
        >
      {/if}
      <Button
        size="sm"
        variant="danger"
        onclick={() =>
          run(async () => {
            const ids = await app.client.tasks.cancel(task.id);
            app.toasts.show({ kind: 'info', message: t('tasks.cancelled', { n: ids.length }) });
          })}>{t('tasks.cancel')}</Button
      >
    {/if}
  </div>
  {#if kids.length}
    <ul class="children">
      {#each kids as child (child.id)}
        <TaskNode task={child} {childrenOf} {titleOf} />
      {/each}
    </ul>
  {/if}
</li>

<style>
  .node {
    display: flex;
    flex-direction: column;
    gap: var(--alfa-space-1);
    padding: var(--alfa-space-2);
    border: 1px solid var(--alfa-color-border);
    border-radius: var(--alfa-radius-card);
    background: var(--alfa-color-surface);
    font-size: var(--alfa-font-size-sm);
  }
  .head {
    display: flex;
    align-items: center;
    gap: var(--alfa-space-2);
  }
  .title {
    flex: 1;
    min-width: 0;
    font-weight: var(--alfa-weight-semibold);
    overflow-wrap: anywhere;
  }
  .status {
    flex: none;
    color: var(--alfa-color-text-muted);
    font-size: var(--alfa-font-size-xs);
  }
  [data-result='failed'] .status {
    color: var(--alfa-color-error);
  }
  [data-result='succeeded'] .status {
    color: var(--alfa-color-success);
  }
  progress {
    width: 100%;
    height: 6px;
    accent-color: var(--alfa-color-info);
  }
  .meta {
    color: var(--alfa-color-text-muted);
    font-size: var(--alfa-font-size-xs);
  }
  .note {
    font-size: var(--alfa-font-size-xs);
    font-style: italic;
  }
  .error {
    color: var(--alfa-color-error);
    font-size: var(--alfa-font-size-xs);
  }
  .actions,
  .steer {
    display: flex;
    flex-wrap: wrap;
    align-items: flex-end;
    gap: var(--alfa-space-2);
  }
  .steer {
    flex: 1 1 100%;
  }
  .steer :global(label) {
    flex: 1;
  }
  .children {
    display: flex;
    flex-direction: column;
    gap: var(--alfa-space-2);
    margin: var(--alfa-space-1) 0 0;
    padding: 0 0 0 var(--alfa-space-3);
    border-left: 2px solid var(--alfa-color-border);
    list-style: none;
  }
</style>
