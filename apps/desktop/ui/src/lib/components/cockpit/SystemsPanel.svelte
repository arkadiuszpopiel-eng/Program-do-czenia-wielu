<!-- Mapa referencyjna offline; żadnych domyślonych lokalizacji lub połączeń na globie. -->
<script lang="ts">
  import { Dialog } from 'bits-ui';
  import { Button, personInfo } from '@alfa/ui-kit';
  import ArrowUpRight from '@lucide/svelte/icons/arrow-up-right';
  import { useApp } from '../../state/context';
  import { openPersonControls } from '../../logic/person-controls';
  import { systemsCopy } from './systems-copy';
  import world from './assets/world-land.svg';
  import './systems.css';

  let { onclose }: { onclose: () => void } = $props();
  const app = useApp();
  const uid = $props.id();
  const copy = $derived(systemsCopy(app.i18n.locale));
  const cast = $derived(app.activeId ? (app.agents[app.activeId] ?? []) : []);
  const profile = $derived(
    app.system?.keys_configured === false
      ? 'local'
      : app.str(
          'models.default_profile',
          app.system?.profile ?? app.sessions.active?.profile ?? '',
        ),
  );
  const network = $derived(
    app.system === null ? copy.unknown : app.system.online ? copy.online : copy.offline,
  );
  const accounts = $derived(
    app.system === null
      ? copy.unknown
      : app.system.keys_configured
        ? copy.configured
        : copy.unconfigured,
  );
  const broker = $derived(
    app.broker.view === null
      ? copy.unknown
      : app.broker.view.state === 'connected'
        ? copy.connected
        : copy.lost,
  );
  const health = $derived(app.work.health ? copy[app.work.health.overall] : copy.unknown);
  function settings(page: string) {
    onclose();
    app.openSettings(page);
  }
</script>

<Dialog.Root open onOpenChange={(open) => !open && onclose()}>
  <Dialog.Portal>
    <Dialog.Overlay class="systems-overlay" />
    <Dialog.Content
      class="systems-panel"
      onOpenAutoFocus={(event) => {
        event.preventDefault();
        document.getElementById(`${uid}-title`)?.focus();
      }}
    >
      <header class="systems-head">
        <div>
          <p class="systems-eyebrow">JARVIS / ATLAS</p>
          <Dialog.Title class="systems-title" id="{uid}-title" tabindex={-1}
            >{copy.title}</Dialog.Title
          >
          <Dialog.Description class="systems-lead">{copy.lead}</Dialog.Description>
        </div>
        <Button variant="secondary" onclick={onclose}>{copy.close}</Button>
      </header>
      <figure class="systems-map">
        <img src={world} alt="" width="720" height="360" />
        <figcaption><span>{copy.map}</span><span>{copy.location}</span></figcaption>
      </figure>
      <p class="systems-map-note">{copy.mapNote}</p>
      <section class="systems-grid" aria-label={copy.systems}>
        <article data-system="profile">
          <h3>{copy.profile}</h3>
          <p>{profile ? app.i18n.tk(`profile.${profile}`) : copy.unknown}</p>
          <small>{copy.profileNote}</small>
        </article>
        <article data-system="network" data-warning={app.system?.online === false}>
          <h3>{copy.network}</h3>
          <p>{network}</p>
        </article>
        <article data-system="accounts">
          <h3>{copy.accounts}</h3>
          <p>{accounts}</p>
          <small>{copy.accountsNote}</small>
        </article>
        <article data-system="broker" data-warning={app.broker.view?.state !== 'connected'}>
          <h3>{copy.broker}</h3>
          <p>{broker}</p>
        </article>
        <article data-system="health" data-warning={app.work.health?.overall !== 'ok'}>
          <h3>{copy.health}</h3>
          <p>{health}</p>
        </article>
        <article data-system="remote">
          <h3>{copy.remote}</h3>
          <p>{copy.disconnected}</p>
          <small>{copy.remoteNote}</small>
        </article>
      </section>
      <section class="systems-crew" aria-label={copy.crew}>
        <h3>{copy.crew}</h3>
        {#if cast.length}
          <div>
            {#each cast as person (person.id)}
              <button
                type="button"
                onclick={() => {
                  onclose();
                  openPersonControls(app, person.id);
                }}
              >
                <span class="systems-avatar" aria-hidden="true">{personInfo(person.id).glyph}</span>
                <span
                  >{personInfo(person.id).name}<small
                    >{app.i18n.t(`agentStatus.${person.status}`)}</small
                  ></span
                >
                <ArrowUpRight size={16} aria-hidden="true" />
              </button>
            {/each}
          </div>
        {:else}<p>{copy.empty}</p>{/if}
      </section>
      <footer class="systems-actions">
        <Button variant="secondary" onclick={() => settings('models')}>{copy.models}</Button>
        <Button variant="secondary" onclick={() => settings('improve')}>{copy.diagnostics}</Button>
        <Button variant="primary" onclick={onclose}>{copy.close}</Button>
      </footer>
    </Dialog.Content>
  </Dialog.Portal>
</Dialog.Root>
