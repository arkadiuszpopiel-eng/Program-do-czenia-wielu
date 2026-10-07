<script lang="ts">
  import { agents, type AgentId } from '../tokens';

  interface Props {
    agent: AgentId;
    /** Rozmiar w px (siatka 4 px). */
    size?: 20 | 24 | 28 | 32 | 40 | 64;
    /** Agentka właśnie mówi — pierścień pulsuje (ruch tylko transform/opacity). */
    speaking?: boolean;
    /** Agentka pracuje w tle — pierścień świeci bez pulsowania. */
    working?: boolean;
    /** Pokazuj imię jako tooltip (domyślnie) i w aria-label. */
    label?: string;
    /** Awatar wewnątrz elementu, który sam ma nazwę (np. przycisku) — ukryty dla czytników. */
    decorative?: boolean;
  }

  let {
    agent,
    size = 28,
    speaking = false,
    working = false,
    label,
    decorative = false,
  }: Props = $props();
  const meta = $derived(agents[agent]);
  const status = $derived(speaking ? 'mówi' : working ? 'pracuje' : undefined);
  const aria = $derived(label ?? `${meta.name}${status ? ` (${status})` : ''}`);
</script>

<span
  class="avatar"
  class:speaking
  class:working
  style:--size="{size}px"
  style:--accent="var(--alfa-agent-{agent})"
  style:--soft="var(--alfa-agent-{agent}-soft)"
  role={decorative ? undefined : 'img'}
  aria-label={decorative ? undefined : aria}
  aria-hidden={decorative ? 'true' : undefined}
  title={decorative ? undefined : aria}
>
  <span class="ring" aria-hidden="true"></span>
  <span class="glyph" aria-hidden="true">{meta.glyph}</span>
</span>

<style>
  .avatar {
    --ring: 2px;
    position: relative;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    flex: none;
    width: var(--size);
    height: var(--size);
    border-radius: var(--alfa-radius-full);
    background: var(--soft);
    color: var(--accent);
    font-weight: var(--alfa-weight-semibold);
    font-size: calc(var(--size) * 0.5);
    line-height: 1;
    isolation: isolate;
  }
  .ring {
    position: absolute;
    inset: 0;
    border-radius: inherit;
    border: var(--ring) solid var(--accent);
    opacity: 0.55;
    transition: opacity var(--alfa-duration-fast) var(--alfa-ease-out);
  }
  .working .ring,
  .speaking .ring {
    opacity: 1;
  }
  .speaking .ring::after {
    content: '';
    position: absolute;
    inset: calc(-1 * var(--ring));
    border-radius: inherit;
    border: var(--ring) solid var(--accent);
    animation: pulse var(--alfa-duration-orb) var(--alfa-ease-out) infinite alternate;
  }
  @keyframes pulse {
    from {
      transform: scale(1);
      opacity: 0.8;
    }
    to {
      transform: scale(1.22);
      opacity: 0;
    }
  }
  .glyph {
    position: relative;
    z-index: 1;
    /* Glif grecki lekko wyżej optycznie. */
    transform: translateY(-4%);
  }
  @media (forced-colors: active) {
    .avatar {
      forced-color-adjust: none;
      background: Canvas;
      color: CanvasText;
    }
    .ring {
      border-color: Highlight;
      opacity: 1;
    }
  }
</style>
