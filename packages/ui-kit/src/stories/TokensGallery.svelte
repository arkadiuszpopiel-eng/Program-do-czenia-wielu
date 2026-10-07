<script lang="ts">
  import { tokens, agentIds, agents } from '../tokens';
  const sizes = Object.entries(tokens.font.size);
  const spaces = Object.entries(tokens.space);
  const neutral = Object.keys(tokens.color.neutral.light).filter((k) => k !== 'scrim');
</script>

<div class="gallery">
  <h2>Typografia</h2>
  {#each sizes as [name, px] (name)}
    <p
      style:font-size="{px}px"
      style:line-height={px >= 20 ? tokens.font.lineHeight.heading : tokens.font.lineHeight.text}
    >
      {name} · {px}px — Zażółć gęślą jaźń. Segoe UI Variable / system-ui
    </p>
  {/each}
  <p style:font-family="var(--alfa-font-mono)">mono · Cascadia Mono / Consolas — const x = 42;</p>

  <h2>Siatka 4 px</h2>
  <div class="row">
    {#each spaces as [name, px] (name)}
      <div class="sp">
        <span class="box" style:width="{px}px" style:height="{px}px"></span><code
          >{name} = {px}</code
        >
      </div>
    {/each}
  </div>

  <h2>Promienie i elewacja</h2>
  <div class="row">
    <div
      class="card"
      style:border-radius="var(--alfa-radius-control)"
      style:box-shadow="var(--alfa-shadow-1)"
    >
      6 / cień 1
    </div>
    <div
      class="card"
      style:border-radius="var(--alfa-radius-card)"
      style:box-shadow="var(--alfa-shadow-2)"
    >
      10 / cień 2
    </div>
    <div
      class="card"
      style:border-radius="var(--alfa-radius-overlay)"
      style:box-shadow="var(--alfa-shadow-3)"
    >
      16 / cień 3
    </div>
    <div class="card" style:border-radius="var(--alfa-radius-full)">pełny</div>
  </div>

  <h2>Neutralne</h2>
  <div class="row">
    {#each neutral as k (k)}
      <div class="swatch">
        <span
          class="color"
          style:background="var(--alfa-color-{k
            .replace(/([a-z0-9])([A-Z])/g, '$1-$2')
            .toLowerCase()})"
        ></span><code>{k}</code>
      </div>
    {/each}
  </div>

  <h2>Agentki</h2>
  <div class="row">
    {#each agentIds as id (id)}
      <div class="swatch">
        <span
          class="color agent"
          style:background="var(--alfa-agent-{id}-soft)"
          style:color="var(--alfa-agent-{id})"
          style:border-color="var(--alfa-agent-{id})">{agents[id].glyph}</span
        >
        <code>{agents[id].name}</code>
      </div>
    {/each}
  </div>

  <h2>Semantyczne i ryzyko</h2>
  <div class="row">
    {#each ['success', 'warning', 'error', 'info'] as k (k)}
      <div class="swatch">
        <span class="color" style:background="var(--alfa-color-{k})"></span><code>{k}</code>
      </div>
    {/each}
    {#each ['low', 'medium', 'high'] as k (k)}
      <div class="swatch">
        <span class="color" style:background="var(--alfa-color-risk-{k})"></span><code
          >risk-{k}</code
        >
      </div>
    {/each}
  </div>
</div>

<style>
  .gallery {
    display: flex;
    flex-direction: column;
    gap: var(--alfa-space-3);
    width: min(760px, 90vw);
  }
  h2 {
    margin-top: var(--alfa-space-4);
    font-size: var(--alfa-font-size-sm);
    letter-spacing: 0.06em;
    text-transform: uppercase;
    color: var(--alfa-color-text-muted);
  }
  .row {
    display: flex;
    flex-wrap: wrap;
    align-items: flex-end;
    gap: var(--alfa-space-3);
  }
  .sp {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: var(--alfa-space-1);
  }
  .box {
    display: block;
    background: var(--alfa-agent-gama);
    border-radius: 2px;
  }
  .card {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 120px;
    height: 72px;
    border: 1px solid var(--alfa-color-border);
    background: var(--alfa-color-surface);
    font-size: var(--alfa-font-size-sm);
  }
  .swatch {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: var(--alfa-space-1);
  }
  .color {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 48px;
    height: 48px;
    border: 1px solid var(--alfa-color-border);
    border-radius: var(--alfa-radius-card);
    font-size: var(--alfa-font-size-xl);
    font-weight: var(--alfa-weight-semibold);
  }
  .agent {
    border-width: 2px;
  }
  code {
    font-size: var(--alfa-font-size-xs);
    color: var(--alfa-color-text-subtle);
  }
</style>
