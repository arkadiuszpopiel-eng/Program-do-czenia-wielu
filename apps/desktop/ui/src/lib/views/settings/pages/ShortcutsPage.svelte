<!-- Skróty (PLAN §14.8): nagrywanie, reset, wyłączenie; konflikty (duplikat, AltGr, zarezerwowane). -->
<script lang="ts">
  import { Button, IconButton, Kbd } from '@alfa/ui-kit';
  import RotateCcw from '@lucide/svelte/icons/rotate-ccw';
  import Ban from '@lucide/svelte/icons/ban';
  import TriangleAlert from '@lucide/svelte/icons/triangle-alert';
  import { SHORTCUTS, SHORTCUT_GROUPS } from '../../../logic/shortcut-registry';
  import { chordFromEvent, findConflicts, type ShortcutConflict } from '../../../logic/shortcuts';
  import { attempt } from '../../../state/attempt';
  import { useApp } from '../../../state/context';

  const app = useApp();
  const { t } = app.i18n;
  let recording = $state<string | null>(null);

  function autofocus(node: HTMLElement) {
    node.focus();
  }

  const conflicts = $derived(findConflicts(SHORTCUTS, app.shortcutOverrides));
  const conflictsOf = (id: string): ShortcutConflict[] =>
    conflicts.filter((c) => c.ids.includes(id));

  function label(id: string): string {
    const goto = /^session\.goto(\d)$/.exec(id);
    return goto
      ? t('shortcut.session.goto', { n: Number(goto[1]) })
      : app.i18n.tk(`shortcut.${id}`);
  }

  function describe(c: ShortcutConflict, id: string): string {
    if (c.reason === 'duplicate')
      return t('sc.conflict.duplicate', {
        other: c.ids
          .filter((x) => x !== id)
          .map(label)
          .join(', '),
      });
    return t(c.reason === 'altgr' ? 'sc.conflict.altgr' : 'sc.conflict.reserved');
  }

  /** `setShortcut` zmienia mapę optymistycznie — po odmowie rdzenia przywracamy poprzedni skrót. */
  async function save(id: string, chord: string | null) {
    const previous = app.shortcutOverrides[id];
    if (await attempt(app.toasts, () => app.setShortcut(id, chord))) return;
    if (previous === undefined) delete app.shortcutOverrides[id];
    else app.shortcutOverrides[id] = previous;
  }

  function record(event: KeyboardEvent, id: string) {
    event.preventDefault();
    event.stopPropagation();
    if (event.key === 'Escape') {
      recording = null;
      return;
    }
    const chord = chordFromEvent(event);
    if (!chord) return;
    recording = null;
    void save(id, chord);
  }
</script>

<p class="intro">{t('sc.intro')}</p>
{#if conflicts.length}
  <p class="warn" role="status">
    <TriangleAlert size={14} strokeWidth={1.5} aria-hidden="true" />
    {t('sc.conflicts', { n: conflicts.length })}
  </p>
{/if}
{#each SHORTCUT_GROUPS as group (group)}
  <section class="group" aria-labelledby="sc-{group}">
    <h3 id="sc-{group}">{app.i18n.tk(`cheatsheet.group.${group}`)}</h3>
    <ul class="list">
      {#each SHORTCUTS.filter((d) => d.group === group) as def (def.id)}
        {@const own = conflictsOf(def.id)}
        <li class="row" class:conflict={own.length > 0}>
          <span class="label">
            {label(def.id)}
            {#if def.scope === 'global'}<span class="tag">{t('sc.global')}</span>{/if}
            {#if !def.customizable}<span class="tag">{t('sc.fixed')}</span>{/if}
            {#each own as c (c.reason + c.chord)}<span class="conflict-text"
                >{describe(c, def.id)}</span
              >{/each}
          </span>
          <span class="chords">
            {#if recording === def.id}
              <button
                type="button"
                class="recorder"
                onkeydown={(e) => record(e, def.id)}
                onblur={() => (recording = null)}
                use:autofocus
              >
                {t('sc.recording')}
              </button>
            {:else}
              {#each app.bindings[def.id] ?? [] as chord (chord)}<Kbd keys={chord} />{:else}<span
                  class="off">{t('cheatsheet.disabled')}</span
                >{/each}
            {/if}
          </span>
          {#if def.customizable}
            <span class="actions">
              <Button
                size="sm"
                variant="secondary"
                onclick={() => (recording = def.id)}
                aria-label={t('sc.record', { label: label(def.id) })}
              >
                {t('sc.recordShort')}
              </Button>
              <IconButton label={t('sc.disable')} size="sm" onclick={() => void save(def.id, '')}>
                <Ban size={14} strokeWidth={1.5} />
              </IconButton>
              <IconButton
                label={t('sc.reset')}
                size="sm"
                disabled={app.shortcutOverrides[def.id] === undefined}
                onclick={() => void save(def.id, null)}
              >
                <RotateCcw size={14} strokeWidth={1.5} />
              </IconButton>
            </span>
          {/if}
        </li>
      {/each}
    </ul>
  </section>
{/each}

<style>
  .intro {
    margin-bottom: var(--alfa-space-3);
    color: var(--alfa-color-text-muted);
    font-size: var(--alfa-font-size-sm);
  }
  .warn {
    display: flex;
    align-items: center;
    gap: var(--alfa-space-1);
    color: var(--alfa-color-warning);
    font-size: var(--alfa-font-size-sm);
    font-weight: var(--alfa-weight-semibold);
  }
  .group {
    margin-bottom: var(--alfa-space-4);
  }
  h3 {
    margin-bottom: var(--alfa-space-2);
    color: var(--alfa-color-text-muted);
    font-size: var(--alfa-font-size-sm);
  }
  .list {
    margin: 0;
    padding: 0;
    list-style: none;
  }
  .row {
    display: flex;
    align-items: center;
    gap: var(--alfa-space-3);
    min-height: 40px;
    padding: var(--alfa-space-1) 0;
    border-bottom: 1px solid var(--alfa-color-border);
    font-size: var(--alfa-font-size-sm);
  }
  .row.conflict {
    background: color-mix(in srgb, var(--alfa-color-warning) 8%, transparent);
  }
  .label {
    display: flex;
    flex-direction: column;
    flex: 1;
    min-width: 0;
  }
  .tag,
  .off {
    color: var(--alfa-color-text-subtle);
    font-size: var(--alfa-font-size-xs);
  }
  .conflict-text {
    color: var(--alfa-color-warning);
    font-size: var(--alfa-font-size-xs);
    font-weight: var(--alfa-weight-semibold);
  }
  .chords {
    display: flex;
    flex-wrap: wrap;
    gap: var(--alfa-space-1);
  }
  .actions {
    display: flex;
    align-items: center;
    gap: 2px;
  }
  .recorder {
    min-height: 28px;
    padding: 0 var(--alfa-space-2);
    border: 1px dashed var(--alfa-color-focus);
    border-radius: var(--alfa-radius-control);
    background: var(--alfa-color-bg);
    color: var(--alfa-color-text);
    font-size: var(--alfa-font-size-xs);
  }
</style>
