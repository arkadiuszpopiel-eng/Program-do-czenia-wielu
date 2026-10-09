// Kamery (`camera-hub`, plan v6 grupa C3): lista kamer, przełączniki i klatka na żądanie.
// Kształty zgodne z `crates/app-api/src/dto` (snake_case, wyliczenia małymi literami).

export type CameraKind = 'usb' | 'network';
export type CameraStateName = 'asleep' | 'opening' | 'live' | 'failed' | 'unplugged';

export interface CameraView {
  readonly id: string;
  readonly name: string;
  readonly kind: CameraKind;
  /** Przełącznik tej kamery; kamera wyłączona nie otworzy się na żadne żądanie. */
  readonly enabled: boolean;
  readonly state: CameraStateName;
  /** Opis błędu przy stanie `failed`. */
  readonly detail: string | null;
  /** Rozmiar obrazu; 0, dopóki kamera nie działa. */
  readonly width: number;
  readonly height: number;
}

export interface CamerasState {
  /** Przełącznik główny: wyłączony = żadna kamera się nie otworzy. */
  readonly enabled: boolean;
  readonly cameras: readonly CameraView[];
}

export interface CameraFrame {
  readonly camera: string;
  readonly width: number;
  readonly height: number;
  /** Chwila zrobienia klatki (ms od 1970). */
  readonly taken_ms: number;
  /** Klatka jako PNG (base64), już pomniejszona do żądanego rozmiaru. */
  readonly png_base64: string;
}
