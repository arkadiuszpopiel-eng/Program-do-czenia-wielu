<script lang="ts">
  import { agentHex, type AgentId, type Theme } from '../tokens';

  interface Props {
    /** Poziom głośności 0–1 (fikcyjny w makiecie; docelowo z VAD/odtwarzacza). */
    level?: number;
    agent?: AgentId;
    size?: number;
    /** Zatrzymuje animację (okno ukryte/zminimalizowane, „bez animacji"). */
    paused?: boolean;
    theme?: Theme;
  }

  let { level = 0, agent = 'alfa', size = 240, paused = false, theme }: Props = $props();

  let canvas = $state<HTMLCanvasElement | null>(null);
  const FPS = 30; // §14.7: orb 30 kl./s

  function resolveTheme(): Theme {
    if (theme) return theme;
    const attr = document.documentElement.getAttribute('data-theme');
    if (attr === 'dark' || attr === 'light') return attr;
    return matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light';
  }

  $effect(() => {
    const el = canvas;
    if (!el) return;
    const ctx = el.getContext('2d');
    if (!ctx) return;
    const dpr = Math.min(window.devicePixelRatio || 1, 2);
    el.width = size * dpr;
    el.height = size * dpr;
    ctx.scale(dpr, dpr);

    const reduced = matchMedia('(prefers-reduced-motion: reduce)').matches;
    let smooth = 0;
    let phase = 0;
    let raf = 0;
    let last = 0;
    let running = true;

    const draw = () => {
      const color = agentHex(agent, resolveTheme());
      const target = Math.max(0, Math.min(1, level));
      smooth += (target - smooth) * 0.25;
      if (!reduced) phase += 0.04 + smooth * 0.08;
      const c = size / 2;
      const base = size * 0.28;
      const r = base + smooth * size * 0.12;
      ctx.clearRect(0, 0, size, size);

      // Poświata (opacity zależna od głośności).
      const glow = ctx.createRadialGradient(c, c, r * 0.6, c, c, r * 1.9);
      glow.addColorStop(0, `${color}55`);
      glow.addColorStop(1, `${color}00`);
      ctx.fillStyle = glow;
      ctx.beginPath();
      ctx.arc(c, c, r * 1.9, 0, Math.PI * 2);
      ctx.fill();

      // Falująca krawędź: suma dwóch sinusoid.
      ctx.beginPath();
      const N = 96;
      for (let i = 0; i <= N; i++) {
        const t = (i / N) * Math.PI * 2;
        const wobble =
          1 +
          (Math.sin(t * 3 + phase) * 0.03 + Math.sin(t * 5 - phase * 1.3) * 0.02) * (0.4 + smooth);
        const x = c + Math.cos(t) * r * wobble;
        const y = c + Math.sin(t) * r * wobble;
        if (i === 0) ctx.moveTo(x, y);
        else ctx.lineTo(x, y);
      }
      ctx.closePath();
      const fill = ctx.createRadialGradient(c - r * 0.3, c - r * 0.3, r * 0.1, c, c, r);
      fill.addColorStop(0, `${color}ff`);
      fill.addColorStop(1, `${color}aa`);
      ctx.fillStyle = fill;
      ctx.fill();
    };

    const loop = (now: number) => {
      if (!running) return;
      raf = requestAnimationFrame(loop);
      if (now - last < 1000 / FPS) return;
      last = now;
      draw();
    };

    const onVisibility = () => {
      const hidden = document.hidden || paused;
      if (hidden) {
        running = false;
        cancelAnimationFrame(raf);
      } else if (!running) {
        running = true;
        raf = requestAnimationFrame(loop);
      }
    };
    document.addEventListener('visibilitychange', onVisibility);
    draw();
    if (!paused) raf = requestAnimationFrame(loop);
    else running = false;

    return () => {
      running = false;
      cancelAnimationFrame(raf);
      document.removeEventListener('visibilitychange', onVisibility);
    };
  });
</script>

<div class="orb-wrap" role="img" aria-label="Wizualizacja głosu, poziom {Math.round(level * 100)}%">
  <canvas
    bind:this={canvas}
    class="orb"
    style:width="{size}px"
    style:height="{size}px"
    aria-hidden="true"
  ></canvas>
</div>

<style>
  .orb-wrap {
    display: inline-block;
    line-height: 0;
  }
  .orb {
    display: block;
  }
</style>
