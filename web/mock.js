// A simulated machine for developing the app without hardware.
// Serve the web/ folder and open index.html?mock (add &speed=4 to fast-forward).
// It mirrors the firmware's REST + WebSocket protocol closely enough to brew,
// edit recipes and walk through calibration.

const params = new URLSearchParams(location.search);
const SPEED = Math.max(0.25, +params.get('speed') || 1);
const TICK_MS = 50;
const REAL_PUMP_GPS = 6.6;   // what the "real" pump does at full power
const TRANSIT_S = 0.4;       // nozzle to scale

export function createMock() {
  const settings = {
    hostname: 'openpour', lastRecipe: '', wifiSsid: '', apMode: true,
    scaleCountsPerGram: 420, pumpGpsAtFull: 6.0, pumpMinDuty: 0.25, pumpLagS: 0.6,
    thetaStepsPerDeg: 8.889, radialStepsPerMm: 80, invertTheta: false, invertRadial: false,
    thetaHomeDeg: -75, radialHomeMm: 66, thetaMinDeg: -78, thetaMaxDeg: 25,
    radialMinMm: 66, radialMaxMm: 175, centerR: 110, centerThetaDeg: 0, parkR: 80, parkThetaDeg: -60,
  };
  let recipes = null;
  const sockets = new Set();

  const sim = {
    state: 'idle', pausedFrom: null, recipe: null, stage: 0, target: 0, inState: 0,
    elapsed: 0, weight: 0, offset: 0, duty: 0, temp: 94.5, homed: false, motion: 'released',
    error: '', message: '', primeLeft: 0, calStep: '',
    inFlight: [], hist: [],
  };

  async function ensureRecipes() {
    if (!recipes) recipes = await (await window.fetch('default-recipes.json')).json();
    return recipes;
  }

  const enter = (s) => { sim.state = s; sim.inState = 0; };
  const stage = () => sim.recipe.stages[sim.stage];
  const total = () => sim.recipe.stages.reduce((a, s) => a + s.water, 0);
  const grams = () => sim.weight - sim.offset;

  function flow() {
    const h = sim.hist;
    if (h.length < 2) return 0;
    const now = h[h.length - 1];
    const old = h.find((p) => now.t - p.t <= 1) ?? h[0];
    return now.t > old.t ? (now.w - old.w) / (now.t - old.t) : 0;
  }

  function broadcast(obj) {
    const data = JSON.stringify(obj);
    for (const s of sockets) s.onmessage?.({ data });
  }
  const notify = (t, msg) => broadcast({ t, msg });

  function beginStage(i) {
    sim.stage = i;
    sim.target += stage().water;
    sim.motion = 'pouring';
    enter(stage().water > 0 ? 'pouring' : 'waiting');
  }

  function fail(msg) {
    sim.duty = 0;
    sim.error = msg;
    sim.motion = 'holding';
    enter('error');
  }

  function finish() {
    sim.duty = 0;
    sim.motion = 'moving';
    enter('finishing');
  }

  function step(dt) {
    sim.inState += dt;
    sim.temp = Math.max(70, sim.temp - 0.004 * dt);

    // Water leaves the nozzle now and lands on the scale TRANSIT_S later.
    if (sim.primeLeft > 0) {
      sim.primeLeft -= dt;
      if (sim.primeLeft <= 0) sim.duty = 0;
    }
    sim.inFlight.push({ at: TRANSIT_S, g: sim.duty * REAL_PUMP_GPS * dt });
    for (const p of sim.inFlight) p.at -= dt;
    while (sim.inFlight.length && sim.inFlight[0].at <= 0) sim.weight += sim.inFlight.shift().g;
    const t = performance.now() / 1000 * SPEED;
    sim.hist.push({ t, w: grams() });
    while (sim.hist.length > 60) sim.hist.shift();

    switch (sim.state) {
      case 'preparing':
        sim.motion = sim.inState < 2 ? 'homing' : 'moving';
        if (!sim.tared) {
          sim.offset = sim.weight;
          sim.tared = true;
        }
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
          if (sim.stage + 1 < sim.recipe.stages.length) beginStage(sim.stage + 1);
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
          sim.offset = sim.weight;
          sim.homed = true;
          sim.duty = 1;
          sim.calStep = 'run';
          sim.inState = 0;
        } else if (sim.calStep === 'run' && sim.inState >= 10) {
          sim.duty = 0;
          sim.calStep = 'settle';
          sim.inState = 0;
        } else if (sim.calStep === 'settle' && sim.inState >= 2) {
          settings.pumpGpsAtFull = +(grams() / 10).toFixed(2);
          sim.message = `Pump calibrated at ${settings.pumpGpsAtFull.toFixed(2)} g/s.`;
          sim.toIdle = true;
          finish();
        }
        break;
    }
  }

  function status() {
    const s = {
      t: 'status', state: sim.state, weight: grams(), flow: flow(), scaleOk: true, duty: sim.duty,
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

  async function command({ cmd, ...a }) {
    switch (cmd) {
      case 'start': {
        if (active()) return notify('error', 'A brew is already running.');
        const r = (await ensureRecipes()).find((x) => x.id === a.recipe);
        if (!r || !r.stages.length) return notify('error', 'That recipe was not found or has no stages.');
        settings.lastRecipe = r.id;
        Object.assign(sim, { recipe: r, stage: 0, target: 0, elapsed: 0, error: '', message: '', toIdle: false, tared: false });
        sim.weight = 380; // a mug and dripper went on the platform
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
    switch (cmd) {
      case 'tare': sim.offset = sim.weight; return;
      case 'home': sim.motion = 'homing'; setTimeout(() => { sim.homed = true; sim.motion = 'holding'; }, 2500 / SPEED); return;
      case 'park': case 'center': case 'jog':
        if (!sim.homed) return notify('error', 'Home the arm first.');
        sim.motion = 'holding';
        return;
      case 'setCenter': return notify('info', 'Dripper centre saved.');
      case 'release': sim.homed = false; sim.motion = 'released'; return;
      case 'prime': sim.duty = a.duty ?? 1; sim.primeLeft = a.seconds ?? 3; return;
      case 'calScale': return notify('info', 'Calibrating the scale. Keep the weight still.');
      case 'calPump':
        sim.message = '';
        sim.calStep = 'prepare';
        sim.weight = 300;
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

  class MockSocket {
    constructor() {
      this.readyState = 0;
      sockets.add(this);
      setTimeout(() => {
        this.readyState = 1;
        this.onopen?.();
        this.onmessage?.({ data: JSON.stringify(status()) });
      }, 150);
    }
    send(data) { command(JSON.parse(data)); }
    close() { sockets.delete(this); this.readyState = 3; this.onclose?.(); }
  }

  const json = (body, status = 200) =>
    new Response(JSON.stringify(body), { status, headers: { 'Content-Type': 'application/json' } });

  async function fetch(url, opts = {}) {
    const path = new URL(url, location.href).pathname.replace(/.*\/api\//, '/api/');
    const method = opts.method || 'GET';
    if (path === '/api/settings') {
      if (method === 'GET') return json(settings);
      const body = JSON.parse(opts.body);
      const wifi = 'wifiSsid' in body || 'wifiPass' in body;
      delete body.wifiPass;
      Object.assign(settings, body);
      setTimeout(() => notify('info', wifi ? 'Wi-Fi saved. Restarting to connect.' : 'Settings saved.'), 100);
      return json({ ok: true });
    }
    if (path === '/api/recipes') {
      if (method === 'GET') return json(await ensureRecipes());
      const body = JSON.parse(opts.body);
      if (!Array.isArray(body)) return json({ error: 'Expected an array of recipes' }, 400);
      recipes = body;
      setTimeout(() => notify('recipes', 'Recipes saved.'), 100);
      return json({ ok: true });
    }
    return window.fetch(url, opts);
  }

  return { Socket: MockSocket, fetch };
}
