<!--
  Awatar agentki. W spoczynku — litera (α β γ δ) w kolorze agentki. Gdy agentka mówi albo pracuje,
  awatar ożywa: pokazuje twarz, która mruga, mówiąc rusza ustami w rytm mowy (poziom z głośnika,
  a bez pomiaru — własny rytm sylab), pracując rozgląda się, a wokół krąży łuk. Duże awatary
  (od 40 px) mają twarz zawsze i literę na plakietce. Styl „same litery” wyłącza twarze —
  zostaje pulsujący pierścień. Mówiąca agentka „wypuszcza” z ust litery tego, co mówi albo pisze
  (właściciel 27.10). Ruch tylko transform/opacity; przy wyłączonych animacjach stan pokazują
  pierścień i kształt ust, a litery nie lecą.
-->
<script lang="ts">
  import { untrack } from 'svelte';
  import { SPEECH_LEVEL_FRESH_MS, avatarState, mouthOpen } from '../avatar-state.svelte';
  import { agents, type AgentId } from '../tokens';

  interface Props {
    agent: AgentId;
    /** Rozmiar w px (siatka 4 px). */
    size?: 20 | 24 | 28 | 32 | 40 | 64;
    /** Agentka właśnie mówi. */
    speaking?: boolean;
    /** Agentka pracuje w tle. */
    working?: boolean;
    /** Pokazuj imię jako tooltip (domyślnie) i w aria-label. */
    label?: string;
    /** Awatar wewnątrz elementu, który sam ma nazwę (np. przycisku) — ukryty dla czytników. */
    decorative?: boolean;
    /** Tekst, który agentka właśnie mówi albo pisze — jego litery wylatują z ust. */
    words?: string;
  }

  let {
    agent,
    size = 28,
    speaking = false,
    working = false,
    label,
    decorative = false,
    words,
  }: Props = $props();
  const meta = $derived(agents[agent]);
  const status = $derived(speaking ? 'mówi' : working ? 'pracuje' : undefined);
  const aria = $derived(label ?? `${meta.name}${status ? ` (${status})` : ''}`);
  const animated = $derived(avatarState.style === 'animated');
  const large = $derived(size >= 40);
  const face = $derived(animated && (large || speaking || working));

  // Usta: gdy przychodzą świeże pomiary poziomu mowy — według nich; inaczej własny rytm (CSS).
  let now = $state(0);
  $effect(() => {
    if (!speaking || !face) return;
    let frame = requestAnimationFrame(function tick(time) {
      now = time;
      frame = requestAnimationFrame(tick);
    });
    return () => cancelAnimationFrame(frame);
  });
  const measured = $derived(
    speaking && avatarState.levelAt > 0 && now - avatarState.levelAt < SPEECH_LEVEL_FRESH_MS,
  );
  const open = $derived(measured ? mouthOpen(avatarState.level) : 0);

  // Litery z ust: co ~420 ms (przy pomiarze — gdy usta są otwarte) wylatuje kolejna litera
  // z końcówki mówionego tekstu; każda żyje 1,2 s. Tekst w pseudoelemencie, poza drzewem dostępności.
  const FALLBACK = 'aoeiu';
  // Z tekstem (wiadomość) litery lecą dopiero, gdy coś już powiedziała — nie w czasie myślenia.
  const pool = $derived.by(() => {
    const tail = (words ?? '').slice(-48).match(/\p{L}/gu) ?? [];
    if (tail.length) return tail.slice(-12).map((c) => c.toLowerCase());
    return words === undefined ? [...FALLBACK] : [];
  });
  let flying = $state<{ id: number; ch: string; at: number; rise: number }[]>([]);
  let spawned = 0;
  let lastSpawn = 0;
  const still = () =>
    document.documentElement.dataset['motion'] === 'off' ||
    window.matchMedia('(prefers-reduced-motion: reduce)').matches;
  $effect(() => {
    const t = now;
    if (!speaking || !face || size < 24) {
      if (untrack(() => flying.length)) flying = [];
      return;
    }
    const alive = untrack(() => flying).filter((p) => t - p.at < 1200);
    const ready = pool.length > 0 && t - lastSpawn > 420 && (!measured || open > 0.3) && !still();
    if (ready) {
      lastSpawn = t;
      spawned += 1;
      const ch = pool[spawned % pool.length] ?? 'a';
      alive.push({ id: spawned, ch, at: t, rise: (spawned % 3) - 1 });
    }
    if (ready || alive.length !== flying.length) flying = alive;
  });
</script>

<span
  class="avatar {agent}"
  class:speaking
  class:working
  class:face
  class:large
  class:measured
  style:--size="{size}px"
  style:--accent="var(--alfa-agent-{agent})"
  style:--soft="var(--alfa-agent-{agent}-soft)"
  style:--open={open}
  role={decorative ? undefined : 'img'}
  aria-label={decorative ? undefined : aria}
  aria-hidden={decorative ? 'true' : undefined}
  title={decorative ? undefined : aria}
>
  <span class="ring" aria-hidden="true"></span>
  {#if working && !speaking}<span class="orbit" aria-hidden="true"></span>{/if}
  {#if face}
    <svg class="mini" viewBox="0 0 32 32" aria-hidden="true">
      <g class="eyes">
        <rect class="eye" x="9.6" y="10.6" width="3.6" height="6" rx="1.8" />
        <rect class="eye" x="18.8" y="10.6" width="3.6" height="6" rx="1.8" />
      </g>
      {#if speaking}
        <ellipse class="mouth talk" cx="16" cy="22.2" rx="3.6" ry="3" />
      {:else if working}
        <rect class="mouth" x="13" y="21.4" width="6" height="1.8" rx="0.9" />
      {:else}
        <path class="smile" d="M12.4 21.2Q16 24.6 19.6 21.2" />
      {/if}
    </svg>
    {#if large}<span class="badge" aria-hidden="true">{meta.glyph}</span>{/if}
    {#each flying as p (p.id)}
      <span class="fly" data-ch={p.ch} style:--rise={p.rise} aria-hidden="true"></span>
    {/each}
  {:else}
    <span class="glyph" aria-hidden="true">{meta.glyph}</span>
  {/if}
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
  /* Mówiąca nad sąsiadkami — litery z ust nie chowają się pod następnym awatarem. */
  .speaking {
    z-index: 2;
  }
  .working .ring,
  .speaking .ring {
    opacity: 1;
  }
  /* Mówi: pierścień pulsuje na zewnątrz. */
  .speaking .ring::after {
    content: '';
    position: absolute;
    inset: calc(-1 * var(--ring));
    border-radius: inherit;
    border: var(--ring) solid var(--accent);
    animation: pulse 900ms var(--alfa-ease-out) infinite;
  }
  @keyframes pulse {
    from {
      transform: scale(1);
      opacity: 0.75;
    }
    to {
      transform: scale(1.3);
      opacity: 0;
    }
  }
  /* Pracuje: jasny łuk krąży po pierścieniu, a całość łagodnie „oddycha”. */
  .working:not(.speaking) .ring {
    opacity: 0.35;
    animation: breathe 1.6s ease-in-out infinite alternate;
  }
  .orbit {
    position: absolute;
    inset: 0;
    border-radius: inherit;
    border: var(--ring) solid transparent;
    border-top-color: var(--accent);
    border-right-color: var(--accent);
    animation: orbit 1.1s linear infinite;
  }
  @keyframes orbit {
    to {
      transform: rotate(360deg);
    }
  }
  @keyframes breathe {
    from {
      opacity: 0.25;
    }
    to {
      opacity: 0.55;
    }
  }
  .glyph {
    position: relative;
    z-index: 1;
    /* Glif grecki lekko wyżej optycznie. */
    transform: translateY(-4%);
  }
  .mini {
    position: relative;
    z-index: 1;
    width: 100%;
    height: 100%;
    fill: var(--accent);
  }
  .eyes {
    transform-box: fill-box;
    transform-origin: center;
    animation: blink 4.6s infinite;
  }
  /* Każda agentka mruga w swoim rytmie. */
  .beta .eyes {
    animation-delay: -1.3s;
  }
  .gama .eyes {
    animation-delay: -2.7s;
  }
  .delta .eyes {
    animation-delay: -3.6s;
  }
  .working:not(.speaking) .eyes {
    animation:
      blink 3.4s infinite,
      look 2.4s ease-in-out infinite alternate;
  }
  @keyframes blink {
    0%,
    93%,
    100% {
      scale: 1 1;
    }
    96% {
      scale: 1 0.12;
    }
  }
  @keyframes look {
    from {
      translate: -1.4px 0;
    }
    to {
      translate: 1.4px 0;
    }
  }
  .smile {
    fill: none;
    stroke: var(--accent);
    stroke-width: 1.8;
    stroke-linecap: round;
  }
  .mouth {
    transform-box: fill-box;
    transform-origin: center;
  }
  /* Usta bez pomiaru: nierówny rytm sylab. */
  .mouth.talk {
    animation: talk 1.15s ease-in-out infinite;
  }
  /* Usta z pomiarem: otwarcie = poziom mowy. */
  .measured .mouth.talk {
    animation: none;
    transform: scaleY(calc(0.14 + var(--open) * 0.86));
    transition: transform 70ms linear;
  }
  @keyframes talk {
    0%,
    100% {
      transform: scaleY(0.2);
    }
    12% {
      transform: scaleY(0.95);
    }
    24% {
      transform: scaleY(0.4);
    }
    38% {
      transform: scaleY(1);
    }
    52% {
      transform: scaleY(0.25);
    }
    64% {
      transform: scaleY(0.8);
    }
    78% {
      transform: scaleY(0.5);
    }
    90% {
      transform: scaleY(0.9);
    }
  }
  /* Litera z ust: wylatuje w bok i lekko w górę, rośnie i gaśnie. */
  .fly {
    position: absolute;
    left: 50%;
    top: 62%;
    z-index: 3;
    pointer-events: none;
    animation: fly 1.2s var(--alfa-ease-out) forwards;
  }
  .fly::before {
    content: attr(data-ch);
    display: block;
    color: var(--accent);
    font-size: max(11px, calc(var(--size) * 0.5));
    font-weight: var(--alfa-weight-semibold);
    line-height: 1;
    text-shadow:
      0 0 3px var(--alfa-color-bg),
      0 0 6px var(--soft);
  }
  @keyframes fly {
    from {
      transform: translate(-50%, -50%) scale(0.4);
      opacity: 0;
    }
    15%,
    60% {
      opacity: 1;
    }
    to {
      transform: translate(calc(var(--size) * 0.8), calc(var(--size) * (-0.3 + var(--rise) * 0.22)))
        scale(1.1);
      opacity: 0;
    }
  }
  .badge {
    position: absolute;
    right: -6%;
    bottom: -6%;
    z-index: 2;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 38%;
    height: 38%;
    border: 1.5px solid var(--alfa-color-bg);
    border-radius: var(--alfa-radius-full);
    background: var(--accent);
    color: var(--alfa-color-bg);
    font-size: calc(var(--size) * 0.24);
  }
  /* Bez animacji: stan pokazują pełny pierścień, łuk w miejscu i kształt ust. */
  :global(:root[data-motion='off']) .avatar :is(.ring, .ring::after, .orbit, .eyes, .mouth) {
    animation: none;
  }
  :global(:root[data-motion='off']) .speaking .ring::after,
  :global(:root[data-motion='off']) .fly {
    display: none;
  }
  @media (prefers-reduced-motion: reduce) {
    .avatar :is(.ring, .orbit, .eyes, .mouth) {
      animation: none;
    }
    .speaking .ring::after,
    .fly {
      display: none;
    }
  }
  @media (forced-colors: active) {
    .avatar {
      forced-color-adjust: none;
      background: Canvas;
      color: CanvasText;
    }
    .ring,
    .orbit {
      border-color: Highlight;
      opacity: 1;
    }
    .mini {
      fill: CanvasText;
    }
    .smile {
      stroke: CanvasText;
    }
  }
</style>
