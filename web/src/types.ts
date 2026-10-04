// The firmware's REST and WebSocket protocol (see README.md, "API"). Field
// names match the JSON exactly.

export type Pattern = 'center' | 'circle' | 'spiral';

export interface Stage {
  name: string;
  /** Grams added in this stage. */
  water: number;
  /** Grams per second. */
  flow: number;
  pattern: Pattern;
  /** mm from the dripper centre. */
  radius: number;
  /** Revolutions per second around the dripper. */
  rps: number;
  /** Seconds to wait after the pour. */
  wait: number;
}

export interface Recipe {
  id: string;
  name: string;
  dose: number;
  minTemp: number;
  stages: Stage[];
}

export interface Settings {
  hostname: string;
  lastRecipe: string;
  wifiSsid: string;
  apMode: boolean;
  flowPulsesPerLitre: number;
  pumpGpsAtFull: number;
  pumpMinDuty: number;
  pumpLagS: number;
  thetaStepsPerDeg: number;
  radialStepsPerMm: number;
  invertTheta: boolean;
  invertRadial: boolean;
  thetaHomeDeg: number;
  radialHomeMm: number;
  thetaMinDeg: number;
  thetaMaxDeg: number;
  radialMinMm: number;
  radialMaxMm: number;
  centerR: number;
  centerThetaDeg: number;
  parkR: number;
  parkThetaDeg: number;
}

export type MachineState =
  | 'idle'
  | 'preparing'
  | 'pouring'
  | 'waiting'
  | 'paused'
  | 'finishing'
  | 'done'
  | 'error'
  | 'calibrating';

export interface Status {
  t: 'status';
  state: MachineState;
  pausedFrom?: MachineState;
  /** Grams through the flow meter since the brew or calibration started. */
  poured: number;
  flow: number;
  duty?: number;
  temp?: number;
  motion?: string;
  homed?: boolean;
  error?: string;
  message?: string;
  // Only while a recipe is running or just finished:
  recipe?: string;
  recipeName?: string;
  stage?: number;
  stages?: number;
  stageName?: string;
  target?: number;
  total?: number;
  targetFlow?: number;
  minTemp?: number;
  elapsed?: number;
  waitLeft?: number;
}

export interface Notice {
  t: 'info' | 'error' | 'recipes';
  msg: string;
}

export type ServerMessage = Status | Notice;

/** A command sent over the WebSocket, with its arguments. */
export type Command =
  | { cmd: 'start'; recipe: string }
  | { cmd: 'pause' | 'resume' | 'stop' | 'home' | 'park' | 'center' | 'setCenter' | 'release' }
  | { cmd: 'jog'; dr: number; dtheta: number }
  | { cmd: 'prime'; seconds?: number; duty?: number }
  | { cmd: 'calPump' | 'meterRun' }
  | { cmd: 'calMeter'; ml: number };

/** What the app needs from a WebSocket; the simulator provides the same. */
export interface SocketLike {
  readonly readyState: number;
  onopen: ((ev: Event) => void) | null;
  onclose: ((ev: CloseEvent) => void) | null;
  onmessage: ((ev: MessageEvent<string>) => void) | null;
  send(data: string): void;
  close(): void;
}

export interface Transport {
  Socket: new (url: string) => SocketLike;
  fetch: (url: string, init?: RequestInit) => Promise<Response>;
}
