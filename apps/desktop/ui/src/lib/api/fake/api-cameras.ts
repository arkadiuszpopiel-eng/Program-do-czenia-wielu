// Atrapa kamer: dwie kamery USB i jedna odłączona. Klatka w przeglądarce to rysowany wzór, który
// zmienia się z każdą klatką (widać, że podgląd żyje); poza przeglądarką — stały mały PNG.
import type { AlfaClient } from '../client';
import type { CameraView, CamerasState } from '../types-cameras';
import type { FakeCore } from './core';

/** PNG 1 × 1 (szary piksel) — klatka zastępcza tam, gdzie nie ma płótna (testy w Node). */
const TINY_PNG =
  'iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR4nGNoaGj4DwAFhAKAjM1mJgAAAABJRU5ErkJggg==';

const SEED: readonly CameraView[] = [
  camera('usb-integrated', 'Integrated Camera', true),
  camera('usb-logi-c920', 'Logitech C920', true),
  { ...camera('usb-old', 'USB2.0 Camera', false), state: 'unplugged' },
];

function camera(id: string, name: string, enabled: boolean): CameraView {
  return { id, name, kind: 'usb', enabled, state: 'asleep', detail: null, width: 0, height: 0 };
}

const HUES: Record<string, number> = { 'usb-integrated': 210, 'usb-logi-c920': 150 };

function draw(id: string, name: string, tick: number, width: number, height: number): string {
  if (typeof document === 'undefined') return TINY_PNG;
  const canvas = document.createElement('canvas');
  canvas.width = width;
  canvas.height = height;
  const ctx = canvas.getContext('2d');
  if (!ctx) return TINY_PNG;
  const hue = HUES[id] ?? 30;
  const sky = ctx.createLinearGradient(0, 0, width, height);
  sky.addColorStop(0, `hsl(${hue} 45% 26%)`);
  sky.addColorStop(1, `hsl(${hue + 40} 40% 12%)`);
  ctx.fillStyle = sky;
  ctx.fillRect(0, 0, width, height);
  // Poruszający się „obiekt” — każda klatka jest inna.
  const x = ((tick * 9) % (width + 80)) - 40;
  ctx.fillStyle = `hsl(${hue + 150} 70% 62%)`;
  ctx.beginPath();
  ctx.arc(x, height * 0.6, height * 0.16, 0, Math.PI * 2);
  ctx.fill();
  ctx.fillStyle = 'rgba(255,255,255,0.86)';
  ctx.font = `${Math.round(height * 0.07)}px "Segoe UI", system-ui, sans-serif`;
  ctx.fillText(`${name} · klatka ${tick}`, 12, height - 14);
  return canvas.toDataURL('image/png').split(',')[1] ?? TINY_PNG;
}

export function camerasApi(core: FakeCore): AlfaClient['cameras'] {
  let cameras: CameraView[] = SEED.map((c) => ({ ...c }));
  let tick = 0;
  const sleepers = new Map<string, number>();
  const master = () => core.settings['camera.enabled'] === true;
  const snapshot = (): CamerasState => ({
    enabled: master(),
    cameras: cameras.map((c) => (master() && c.enabled ? c : asleep(c))),
  });
  const asleep = (c: CameraView): CameraView =>
    c.state === 'unplugged' ? c : { ...c, state: 'asleep', width: 0, height: 0 };
  const patch = (id: string, change: Partial<CameraView>): void => {
    cameras = cameras.map((c) => (c.id === id ? { ...c, ...change } : c));
  };
  const publish = (): Promise<CamerasState> => {
    const state = snapshot();
    core.emit([{ type: 'CamerasChanged', state }]);
    return core.reply(state);
  };
  const fail = (message: string) => Promise.reject(new Error(message));

  return {
    state: () => core.reply(snapshot()),
    setMaster: (enabled) => {
      core.settings = { ...core.settings, 'camera.enabled': enabled };
      return publish();
    },
    setEnabled: (id, enabled) => {
      patch(id, { enabled });
      return publish();
    },
    rename: (id, name) => {
      const clean = name.trim();
      if (clean) patch(id, { name: clean });
      return publish();
    },
    refresh: () => publish(),
    frame: (id, maxSide) => {
      const found = cameras.find((c) => c.id === id);
      if (!found) return fail('Nie ma takiej kamery.');
      if (!master()) return fail('Kamery są wyłączone.');
      if (!found.enabled) return fail('Ta kamera jest wyłączona.');
      if (found.state === 'unplugged') return fail('Kamera jest odłączona.');
      const side = Math.max(64, Math.min(1280, Math.round(maxSide)));
      const [width, height] = [side, Math.round((side * 9) / 16)];
      if (found.state !== 'live') {
        patch(id, { state: 'live', width: 1280, height: 720 });
        core.emit([{ type: 'CamerasChanged', state: snapshot() }]);
      }
      // Bez kolejnych żądań kamera zasypia (jak w programie).
      const scheduler = core.scheduler;
      const previous = sleepers.get(id);
      if (previous !== undefined) scheduler.clearTimeout(previous);
      sleepers.set(
        id,
        scheduler.setTimeout(() => {
          patch(id, { state: 'asleep', width: 0, height: 0 });
          core.emit([{ type: 'CamerasChanged', state: snapshot() }]);
        }, 4_000),
      );
      tick += 1;
      return core.reply({
        camera: id,
        width,
        height,
        taken_ms: scheduler.now(),
        png_base64: draw(id, found.name, tick, width, height),
      });
    },
  };
}
