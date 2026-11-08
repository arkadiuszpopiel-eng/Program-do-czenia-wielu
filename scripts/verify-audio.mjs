#!/usr/bin/env node
// Nagrania kontrolne do testów sprzętowych (docs/plan/24): mowa SYNTETYCZNA (nigdy głos prawdziwej
// osoby) przeliczona do 16 kHz mono 16 bit — tego formatu oczekują VAD, rozpoznawanie i model mówcy.
// Źródło: nagrania głosu Supertonic z prób (poza repozytorium); wynik: target/verify/audio/.
//
//   node scripts/verify-audio.mjs [katalog-źródłowy] [katalog-wyniku]
import { existsSync, mkdirSync, readFileSync, readdirSync, writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = join(dirname(fileURLToPath(import.meta.url)), '..');
const source = process.argv[2] ?? join(root, '..', 'alfa-glos-proba', 'wyniki', 'supertonic3');
const out = process.argv[3] ?? join(root, 'target', 'verify', 'audio');
const RATE = 16_000;

/** Odczyt WAV PCM 16 bit: próbki pierwszego kanału i częstotliwość. */
function readWav(path) {
  const data = readFileSync(path);
  if (data.toString('ascii', 0, 4) !== 'RIFF' || data.toString('ascii', 8, 12) !== 'WAVE') {
    throw new Error(`${path}: to nie jest plik WAV`);
  }
  let offset = 12;
  let format = null;
  while (offset + 8 <= data.length) {
    const id = data.toString('ascii', offset, offset + 4);
    const size = data.readUInt32LE(offset + 4);
    const body = offset + 8;
    if (id === 'fmt ') {
      format = {
        code: data.readUInt16LE(body),
        channels: data.readUInt16LE(body + 2),
        rate: data.readUInt32LE(body + 4),
        bits: data.readUInt16LE(body + 14),
      };
    } else if (id === 'data') {
      if (!format || format.code !== 1 || format.bits !== 16) {
        throw new Error(`${path}: obsługiwany jest tylko PCM 16 bit`);
      }
      const frames = Math.floor(size / (2 * format.channels));
      const samples = new Float64Array(frames);
      for (let i = 0; i < frames; i++)
        samples[i] = data.readInt16LE(body + i * 2 * format.channels);
      return { samples, rate: format.rate };
    }
    offset = body + size + (size % 2);
  }
  throw new Error(`${path}: brak danych dźwięku`);
}

/** Przeliczenie częstotliwości: uśrednienie okna (filtr przeciw nakładaniu widma) + interpolacja. */
function resample(samples, from, to) {
  if (from === to) return samples;
  const ratio = from / to;
  const half = Math.max(1, Math.round(ratio / 2));
  const smooth = new Float64Array(samples.length);
  let sum = 0;
  for (let i = 0; i < samples.length + half; i++) {
    if (i < samples.length) sum += samples[i];
    if (i - 2 * half - 1 >= 0) sum -= samples[i - 2 * half - 1];
    const center = i - half;
    if (center >= 0 && center < samples.length) {
      const width = Math.min(i, samples.length - 1) - Math.max(0, i - 2 * half) + 1;
      smooth[center] = sum / width;
    }
  }
  const length = Math.floor(samples.length / ratio);
  const result = new Float64Array(length);
  for (let i = 0; i < length; i++) {
    const position = i * ratio;
    const left = Math.floor(position);
    const right = Math.min(left + 1, smooth.length - 1);
    result[i] = smooth[left] + (smooth[right] - smooth[left]) * (position - left);
  }
  return result;
}

/** Bez ciszy na początku i końcu (próg 1 % skali, zapas 20 ms) — testy zakładają mowę od pierwszej próbki. */
function trimSilence(samples) {
  const threshold = 328;
  let first = 0;
  let last = samples.length - 1;
  while (first < samples.length && Math.abs(samples[first]) < threshold) first++;
  while (last > first && Math.abs(samples[last]) < threshold) last--;
  const margin = RATE / 50;
  return samples.slice(Math.max(0, first - margin), Math.min(samples.length, last + margin));
}

function writeWav(path, samples) {
  const header = Buffer.alloc(44);
  header.write('RIFF', 0, 'ascii');
  header.writeUInt32LE(36 + samples.length * 2, 4);
  header.write('WAVEfmt ', 8, 'ascii');
  header.writeUInt32LE(16, 16);
  header.writeUInt16LE(1, 20);
  header.writeUInt16LE(1, 22);
  header.writeUInt32LE(RATE, 24);
  header.writeUInt32LE(RATE * 2, 28);
  header.writeUInt16LE(2, 32);
  header.writeUInt16LE(16, 34);
  header.write('data', 36, 'ascii');
  header.writeUInt32LE(samples.length * 2, 40);
  const body = Buffer.alloc(samples.length * 2);
  for (let i = 0; i < samples.length; i++) {
    body.writeInt16LE(Math.max(-32768, Math.min(32767, Math.round(samples[i]))), i * 2);
  }
  writeFileSync(path, Buffer.concat([header, body]));
}

if (!existsSync(source)) {
  console.error(`Brak katalogu z nagraniami: ${source}`);
  process.exit(2);
}
const files = readdirSync(source)
  .filter((name) => /\.wav$/i.test(name))
  .sort()
  .slice(0, 5);
if (files.length === 0) {
  console.error(`W ${source} nie ma plików WAV.`);
  process.exit(2);
}
mkdirSync(out, { recursive: true });
files.forEach((name, index) => {
  const { samples, rate } = readWav(join(source, name));
  const target = join(out, `mowa-0${index + 1}-16k.wav`);
  const converted = trimSilence(resample(samples, rate, RATE));
  writeWav(target, converted);
  console.log(`${name} (${rate} Hz) → ${target} (${(converted.length / RATE).toFixed(1)} s)`);
});
