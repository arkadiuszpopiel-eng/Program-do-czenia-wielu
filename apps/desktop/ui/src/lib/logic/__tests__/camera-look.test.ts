import { describe, expect, it } from 'vitest';
import type { CameraView } from '../../api/types-cameras';
import { cameraFor, plain, wantsCameraLook } from '../camera-look';

function camera(id: string, name: string, patch: Partial<CameraView> = {}): CameraView {
  return {
    id,
    name,
    kind: 'usb',
    enabled: true,
    state: 'asleep',
    detail: null,
    width: 0,
    height: 0,
    ...patch,
  };
}

describe('„opisz mi to, co widzisz”', () => {
  it('rozpoznaje samą prośbę oraz prośby, w których pada słowo „kamera”', () => {
    for (const text of [
      'Opisz mi to, co widzisz',
      'opisz mi to co widzisz',
      'Opisz co widzisz.',
      'Alfa, opisz mi to, co teraz widzisz!',
      'Powiedz mi, co widzisz',
      'Co widzisz?',
      'co teraz widzisz',
      'Co widzisz na kamerze w kuchni?',
      'Co jest na kamerze?',
      'Co widać na kamerce przy drzwiach?',
      'Spójrz na kamerę i powiedz, czy ktoś tam jest',
      'popatrz przez kamere',
      'Kto jest przy drzwiach? Sprawdź kamerę.',
      'Describe what you see',
      'What do you see?',
      'What do you see on the camera?',
      'Look at the webcam and describe the room',
    ]) {
      expect(wantsCameraLook(text), text).toBe(true);
    }
  });

  it('zwykłe zdania nie robią zdjęcia — także te z „co widzisz” o kodzie, pliku czy zrzucie', () => {
    for (const text of [
      'Widzisz, że mam rację.',
      'Jak widzisz, raport jest gotowy.',
      'Opisz mi plan wyjazdu',
      'Napisz, co o tym sądzisz',
      'Czy widzisz błąd w tym kodzie?',
      'Co widzisz w tym kodzie?',
      'Co widzisz w tym pliku i w logu?',
      'Opisz, co widzisz na tym zrzucie ekranu',
      'Opisz mi to, co widzisz w załączniku',
      'To, co widzisz w załączniku, to wykres sprzedaży',
      'What do you see wrong in this code?',
      'Describe what you see in the attached chart',
      'Kto jest autorem tego raportu?',
      'Zerknij i opisz, co jest w tabeli',
      '/opisz co widzisz',
      '',
      `opisz mi to co widzisz na kamerze ${'x'.repeat(300)}`,
    ]) {
      expect(wantsCameraLook(text), text).toBe(false);
    }
  });

  it('wybiera kamerę nazwaną w prośbie, a bez nazwy pierwszą włączoną', () => {
    const cameras = [
      camera('a', 'Biurko'),
      camera('b', 'Kuchnia'),
      camera('c', 'Drzwi', { enabled: false }),
      camera('d', 'Garaż', { state: 'unplugged' }),
    ];
    expect(cameraFor('Opisz mi to, co widzisz', cameras)?.id).toBe('a');
    expect(cameraFor('Co widzisz na kamerze kuchnia?', cameras)?.id).toBe('b');
    // Wyłączona i odłączona kamera nie są brane, nawet nazwane.
    expect(cameraFor('Co jest na kamerze Drzwi?', cameras)?.id).toBe('a');
    expect(cameraFor('spójrz na kamerę garaż', cameras)?.id).toBe('a');
    expect(cameraFor('opisz co widzisz', [])).toBeNull();
    expect(cameraFor('opisz co widzisz', [camera('x', 'X', { enabled: false })])).toBeNull();
    expect(plain('  Żółć, Łódź!  ')).toBe('zolc lodz');
  });
});
