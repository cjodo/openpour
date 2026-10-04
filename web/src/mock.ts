// A simulated machine for developing the app without hardware.
// `npm run dev` and open http://localhost:8000/?mock (add &speed=4 to fast-forward).
// It mirrors the firmware's REST + WebSocket protocol closely enough to brew,
// edit recipes and walk through calibration. Left out of firmware builds.

import type { Command, MachineState, Notice, Recipe, Settings, SocketLike, Stage, Status, Transport } from './types';

const params = new URLSearchParams(location.search);
const SPEED = Math.max(0.25, +(params.get('speed') ?? 1) || 1);
const TICK_MS = 50;
const REAL_PUMP_GPS = 6.6;   // what the "real" pump does at full power
const REAL_METER_PPL = 2010; // what the "real" flow meter does (settings start at the datasheet value)
const METER_LAG_S = 0.1;     // pump coast-down after it is switched off

export function createMock(): Transport {
  const settings: Settings = {
    hostname: 'openpour', lastRecipe: '', wifiSsid: '', apMode: true,
    flowPulsesPerLitre: 1925, pumpGpsAtFull: 6.0, pumpMinDuty: 0.25, pumpLagS: 0.15,
    telemetryHz: 10, logDebug: false,
    thetaStepsPerDeg: 8.889, radialStepsPerMm: 80, invertTheta: false, invertRadial: false,
    thetaHomeDeg: -75, radialHomeMm: 66, thetaMinDeg: -78, thetaMaxDeg: 25,
    radialMinMm: 66, radialMaxMm: 175, centerR: 110, centerThetaDeg: 0, parkR: 80, parkThetaDeg: -60,
  };
  let recipes: Recipe[] | null = null;
  const sockets = new Set<MockSocket>();

  const sim = {
    state: 'idle' as MachineState,
    pausedFrom: 'idle' as MachineState,
    pausedIn: 0,
    recipe: null as Recipe | null,
    stage: 0, target: 0, inState: 0,
    elapsed: 0, pulses: 0, duty: 0, temp: 94.5, homed: false, motion: 'released',
    error: '', message: '', primeLeft: 0, toIdle: false,
    calStep: '' as '' | 'prepare' | 'run' | 'settle',
    calKind: '' as '' | 'pump' | 'meter',
    coast: 0,
    hist: [] as { t: number; w: number }[],
  };

  async function ensureRecipes(): Promise<Recipe[]> {
    if (!recipes) recipes = (await (await window.fetch('default-recipes.json')).json()) as Recipe[];
    return recipes;
  }

  const enter = (s: MachineState) => { sim.state = s; sim.inState = 0; };
  const stage = (): Stage => sim.recipe!.stages[sim.stage];
  const total = () => sim.recipe!.stages.reduce((a, s) => a + s.water, 0);
  const grams = () => (sim.pulses * 1000) / settings.flowPulsesPerLitre;
  const resetMeter = () => { sim.pulses = 0; sim.hist = []; };

  function flow() {
    const h = sim.hist;
    if (h.length < 2) return 0;
    const now = h[h.length - 1];
    const old = h.find((p) => now.t - p.t <= 1) ?? h[0];
    return now.t > old.t ? (now.w - old.w) / (now.t - old.t) : 0;
  }

  function broadcast(obj: Status | Notice) {
    const data = JSON.stringify(obj);
    for (const s of sockets) s.onmessage?.(new MessageEvent('message', { data }));
  }
  const notify = (t: Notice['t'], msg: string) => broadcast({ t, msg });

  function beginStage(i: number) {
    sim.stage = i;
    sim.target += stage().water;
    sim.motion = 'pouring';
    enter(stage().water > 0 ? 'pouring' : 'waiting');
  }

  function finish() {
    sim.duty = 0;
    sim.motion = 'moving';
    enter('finishing');
  }

  function step(dt: number) {
    sim.inState += dt;
    sim.temp = Math.max(70, sim.temp - 0.004 * dt);

    // The meter counts water as it is pumped; the pump coasts briefly after stopping.
    if (sim.primeLeft > 0) {
      sim.primeLeft -= dt;
      if (sim.primeLeft <= 0) sim.duty = 0;
    }
    sim.coast = sim.duty > 0 ? sim.duty : Math.max(0, sim.coast - dt / METER_LAG_S);
    sim.pulses += (Math.max(sim.duty, sim.coast) * REAL_PUMP_GPS * dt * REAL_METER_PPL) / 1000;
    const t = performance.now() / 1000 * SPEED;
    sim.hist.push({ t, w: grams() });
    while (sim.hist.length > 60) sim.hist.shift();

    switch (sim.state) {
      case 'preparing':
        sim.motion = sim.inState < 2 ? 'homing' : 'moving';
        if (sim.inState >= 3) {
          sim.homed = true;
          beginStage(0);
        }
        break;
      case 'pouring': {
        sim.elapsed += dt;
        const f = flow();
        if (grams() + Math.max(0, f) * settings.pumpLagS >= sim.target) {
          sim.duty = 0;
          sim.motion = 'holding';
          enter('waiting');
        } else {
          sim.duty = Math.min(1, Math.max(settings.pumpMinDuty, stage().flow / settings.pumpGpsAtFull));
        }
        break;
      }
      case 'waiting':
        sim.elapsed += dt;
        if (sim.inState >= stage().wait) {
          if (sim.stage + 1 < sim.recipe!.stages.length) beginStage(sim.stage + 1);
          else finish();
        }
        break;
      case 'finishing':
        if (sim.inState >= 1.5) {
          sim.motion = 'released';
          sim.homed = false;
          enter(sim.toIdle ? 'idle' : 'done');
        }
        break;
      case 'calibrating':
        if (sim.calStep === 'prepare' && sim.inState >= 2) {
          resetMeter();
          sim.homed = true;
          sim.duty = 1;
          sim.calStep = 'run';
          sim.inState = 0;
        } else if (sim.calStep === 'run' && (sim.calKind === 'meter' ? grams() >= 200 : sim.inState >= 10)) {
          sim.duty = 0;
          sim.calStep = 'settle';
          sim.inState = 0;
        } else if (sim.calStep === 'settle' && sim.inState >= 2) {
          if (sim.calKind === 'meter') {
            const real = (sim.pulses * 1000) / REAL_METER_PPL;
            sim.message = `The meter counted ${grams().toFixed(0)} ml. Weigh or measure the water and enter the real amount.`;
            console.info(`[mock] the jug really holds ${real.toFixed(0)} ml`);
          } else {
            settings.pumpGpsAtFull = +(grams() / 10).toFixed(2);
            sim.message = `Pump calibrated at ${settings.pumpGpsAtFull.toFixed(2)} g/s.`;
          }
          sim.toIdle = true;
          finish();
        }
        break;
    }
  }

  function status(): Status {
    const s: Status = {
      t: 'status', state: sim.state, poured: grams(), flow: flow(), duty: sim.duty,
      temp: sim.temp, motion: sim.motion, homed: sim.homed,
    };
    if (sim.state === 'paused') s.pausedFrom = sim.pausedFrom;
    if (sim.recipe && !['idle', 'calibrating'].includes(sim.state)) {
      Object.assign(s, {
        recipe: sim.recipe.id, recipeName: sim.recipe.name, stage: sim.stage, stages: sim.recipe.stages.length,
        stageName: stage().name, target: sim.target, total: total(), targetFlow: stage().flow,
        minTemp: sim.recipe.minTemp, elapsed: sim.elapsed,
      });
      const waiting = sim.state === 'waiting' || (sim.state === 'paused' && sim.pausedFrom === 'waiting');
      if (waiting) s.waitLeft = Math.max(0, stage().wait - sim.inState);
    }
    if (sim.error) s.error = sim.error;
    if (sim.message) s.message = sim.message;
    return s;
  }

  const active = () => !['idle', 'done', 'error'].includes(sim.state);

  async function command(c: Command) {
    switch (c.cmd) {
      case 'start': {
        if (active()) return notify('error', 'A brew is already running.');
        const r = (await ensureRecipes()).find((x) => x.id === c.recipe);
        if (!r || !r.stages.length) return notify('error', 'That recipe was not found or has no stages.');
        settings.lastRecipe = r.id;
        Object.assign(sim, { recipe: r, stage: 0, target: 0, elapsed: 0, error: '', message: '', toIdle: false });
        resetMeter();
        enter('preparing');
        return;
      }
      case 'pause':
        if (!['pouring', 'waiting', 'preparing'].includes(sim.state)) return;
        sim.pausedFrom = sim.state;
        sim.pausedIn = sim.inState;
        sim.duty = 0;
        sim.state = 'paused';
        return;
      case 'resume':
        if (sim.state !== 'paused') return;
        sim.state = sim.pausedFrom;
        sim.inState = sim.pausedIn;
        return;
      case 'stop':
        sim.duty = 0;
        if (sim.state === 'done' || sim.state === 'error') {
          sim.error = '';
          return enter('idle');
        }
        if (active()) {
          sim.toIdle = true;
          finish();
        }
        return;
    }
    if (active()) return notify('error', "That isn't available while brewing.");
    switch (c.cmd) {
      case 'home': sim.motion = 'homing'; setTimeout(() => { sim.homed = true; sim.motion = 'holding'; }, 2500 / SPEED); return;
      case 'park': case 'center': case 'jog':
        if (!sim.homed) return notify('error', 'Home the arm first.');
        sim.motion = 'holding';
        return;
      case 'setCenter': return notify('info', 'Dripper centre saved.');
      case 'release': sim.homed = false; sim.motion = 'released'; return;
      case 'prime': sim.duty = c.duty ?? 1; sim.primeLeft = c.seconds ?? 3; return;
      case 'calMeter': {
        const ml = +c.ml;
        if (!(sim.pulses >= 50 && ml >= 10)) return notify('error', 'Dispense some water first, then enter how much came out.');
        settings.flowPulsesPerLitre = Math.round((sim.pulses * 1000) / ml);
        return notify('info', `Flow meter calibrated at ${settings.flowPulsesPerLitre} pulses per litre.`);
      }
      case 'calPump': case 'meterRun':
        sim.message = '';
        sim.calStep = 'prepare';
        sim.calKind = c.cmd === 'meterRun' ? 'meter' : 'pump';
        return enter('calibrating');
    }
  }

  let last = performance.now();
  let sinceStatus = 0;
  setInterval(() => {
    const now = performance.now();
    const dt = ((now - last) / 1000) * SPEED;
    last = now;
    step(dt);
    sinceStatus += dt / SPEED;
    if (sinceStatus >= 0.2) {
      sinceStatus = 0;
      broadcast(status());
    }
  }, TICK_MS);

  class MockSocket implements SocketLike {
    readyState: number = WebSocket.CONNECTING;
    onopen: SocketLike['onopen'] = null;
    onclose: SocketLike['onclose'] = null;
    onmessage: SocketLike['onmessage'] = null;

    constructor(_url: string) {
      sockets.add(this);
      setTimeout(() => {
        this.readyState = WebSocket.OPEN;
        this.onopen?.(new Event('open'));
        this.onmessage?.(new MessageEvent('message', { data: JSON.stringify(status()) }));
      }, 150);
    }
    send(data: string) { command(JSON.parse(data) as Command); }
    close() { sockets.delete(this); this.readyState = WebSocket.CLOSED; this.onclose?.(new CloseEvent('close')); }
  }

  const json = (body: unknown, status = 200) =>
    new Response(JSON.stringify(body), { status, headers: { 'Content-Type': 'application/json' } });

  async function fetch(url: string, opts: RequestInit = {}): Promise<Response> {
    const path = new URL(url, location.href).pathname.replace(/.*\/api\//, '/api/');
    const method = opts.method || 'GET';
    if (path === '/api/settings') {
      if (method === 'GET') return json(settings);
      const body = JSON.parse(String(opts.body)) as Partial<Settings> & { wifiPass?: string };
      const wifi = 'wifiSsid' in body || 'wifiPass' in body;
      delete body.wifiPass;
      Object.assign(settings, body);
      setTimeout(() => notify('info', wifi ? 'Wi-Fi saved. Restarting to connect.' : 'Settings saved.'), 100);
      return json({ ok: true });
    }
    if (path === '/api/recipes') {
      if (method === 'GET') return json(await ensureRecipes());
      const body: unknown = JSON.parse(String(opts.body));
      if (!Array.isArray(body)) return json({ error: 'Expected an array of recipes' }, 400);
      recipes = body as Recipe[];
      setTimeout(() => notify('recipes', 'Recipes saved.'), 100);
      return json({ ok: true });
    }
    return window.fetch(url, opts);
  }

  return { Socket: MockSocket, fetch };
}
