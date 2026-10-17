import { describe, expect, it } from 'vitest';
import {
  CARD_HEADROOM_MB,
  INTEGRATED_MAX_MB,
  UMA_POOL_MIN_MB,
  cardFit,
  chatMemoryMb,
  mainCardMb,
} from '../models';

const MIB = 1024 * 1024;

describe('ocena „mieści się na karcie”', () => {
  it('karta główna to największa karta niezintegrowana', () => {
    expect(
      mainCardMb([
        { vram_mb: 128, integrated: true },
        { vram_mb: 6144, integrated: false },
        { vram_mb: 16384, integrated: false },
      ]),
    ).toBe(16384);
    // Procesor z grafiką: mała rezerwa z BIOS-u to RAM, duża (≥ 6 GB) — własna pula jak karta.
    expect(mainCardMb([{ vram_mb: 4096, integrated: true }])).toBeNull();
    expect(mainCardMb([{ vram_mb: 16384, integrated: true }])).toBe(16384);
    expect(mainCardMb([{ vram_mb: UMA_POOL_MIN_MB - 1, integrated: true }])).toBeNull();
    expect(mainCardMb([])).toBeNull();
  });

  it('przy APU z dużą rezerwą i karcie osobnej liczy się karta osobna (jak profil w Rust)', () => {
    // Ryzen AI z 16 GB rezerwy i RTX 4060 8 GB: model rozmowy trafia na RTX — ocena też.
    const gpus = [
      { vram_mb: 16384, integrated: true },
      { vram_mb: 8188, integrated: false },
    ];
    expect(mainCardMb(gpus)).toBe(8188);
    expect(cardFit(10 * 1024 * MIB, mainCardMb(gpus))).toBe('partly');
  });

  it('sama grafika zintegrowana klasy „karta zintegrowana”: model liczy na niej, nie na procesorze', () => {
    // Core Ultra 7 258V (Arc 140V, 128 MB + 16 GB współdzielonej), 32 GB RAM — budżet jak w Rust.
    const profile = (hw_class: string, ram_mb = 32_768) => ({
      machine: { ram_mb, gpus: [{ vram_mb: 128, shared_mb: 16_000, integrated: true }] },
      recommendation: { hw_class },
    });
    expect(chatMemoryMb(profile('integrated'))).toBe(INTEGRATED_MAX_MB);
    expect(cardFit(1100 * MIB, chatMemoryMb(profile('integrated')))).toBe('fits');
    // 12 GB RAM: najwyżej ćwierć RAM; inna klasa (np. „nieznana”) — model liczy procesor.
    expect(chatMemoryMb(profile('integrated', 12_288))).toBe(3072);
    expect(chatMemoryMb(profile('unknown'))).toBeNull();
  });

  it('plik z zapasem 1,5 GB musi zmieścić się w pamięci karty', () => {
    // Bielik 1.5B (1620 MiB) na karcie 6 GB — cały.
    expect(cardFit(1620 * MIB, 6144)).toBe('fits');
    // Bielik 4.5B (4826 MiB) na karcie 6 GB — tylko część warstw; na 8 GB — cały.
    expect(cardFit(4826 * MIB, 6144)).toBe('partly');
    expect(cardFit(4826 * MIB, 8192)).toBe('fits');
    // Dokładnie na granicy mieści się.
    expect(cardFit((6144 - CARD_HEADROOM_MB) * MIB, 6144)).toBe('fits');
    expect(cardFit((6144 - CARD_HEADROOM_MB + 1) * MIB, 6144)).toBe('partly');
  });

  it('bez karty albo z bardzo małą kartą liczy procesor', () => {
    expect(cardFit(1620 * MIB, null)).toBe('cpu');
    expect(cardFit(1620 * MIB, 1024)).toBe('cpu');
  });
});
