<!--
  Znak Alfy: zaokrąglony kwadrat z przejściem fiolet → błękit i białą literą α. Ten sam rysunek
  jest ikoną programu (`src-tauri/icons/alfa-mark.svg`) — zmieniając jeden, zmień drugi.
-->
<script lang="ts">
  interface Props {
    size?: number;
  }
  let { size = 18 }: Props = $props();
  const id = $props.id();
</script>

<svg class="mark" viewBox="0 0 64 64" width={size} height={size} aria-hidden="true">
  <defs>
    <linearGradient id="{id}-g" x1="0" y1="0" x2="1" y2="1">
      <stop offset="0" stop-color="#7c5cff" />
      <stop offset="0.55" stop-color="#4f46e5" />
      <stop offset="1" stop-color="#0ea5c6" />
    </linearGradient>
  </defs>
  <rect x="2" y="2" width="60" height="60" rx="17" fill="url(#{id}-g)" />
  <path
    d="M46 19.5C40.5 33 33.5 45 24.5 45 17.5 45 13.5 39 13.5 32S17.5 19 24.5 19C33.5 19 40 31 47.5 45"
    fill="none"
    stroke="#ffffff"
    stroke-width="5.5"
    stroke-linecap="round"
    stroke-linejoin="round"
  />
</svg>

<style>
  .mark {
    display: block;
    flex: none;
  }
  @media (forced-colors: active) {
    .mark {
      forced-color-adjust: none;
    }
  }
</style>
