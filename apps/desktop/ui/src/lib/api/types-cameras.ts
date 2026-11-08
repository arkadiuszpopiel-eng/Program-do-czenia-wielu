// Kamery (`camera-hub`, plan v6 grupa C3): lista kamer, przełączniki i klatka na żądanie.
// Kształty zgodne z `crates/app-api/src/dto` (snake_case, wyliczenia małymi literami).

import type { CameraFrame, CameraKindView, CameraStateView, CamerasState } from './generated/ipc';
export type { CameraFrame, CameraView, CamerasState } from './generated/ipc';

export type CameraKind = CameraKindView;
export type CameraStateName = CameraStateView;

/** Kamery (część `AlfaClient`: `cameras_*`). */
export interface CamerasApi {
  state(): Promise<CamerasState>;
  /** Przełącznik główny (to samo co ustawienie `camera.enabled`). */
  setMaster(enabled: boolean): Promise<CamerasState>;
  setEnabled(id: string, enabled: boolean): Promise<CamerasState>;
  rename(id: string, name: string): Promise<CamerasState>;
  /** Ponowne wyszukanie kamer (podłączone / odłączone). */
  refresh(): Promise<CamerasState>;
  /** Jedna bieżąca klatka; budzi kamerę, jeśli śpi. `maxSide`: dłuższy bok w pikselach. */
  frame(id: string, maxSide: number): Promise<CameraFrame>;
}
