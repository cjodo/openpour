// OpenPour control app. Bundled by build.mjs; `npm run dev` serves it with the
// simulated machine (see mock.ts) at http://localhost:8000/?mock.

import type {
  Command, MachineState, Pattern, Recipe, ServerMessage, Settings, SocketLike, Stage, Status, Transport,
} from './types';

function $<T extends Element = HTMLElement>(sel: string, root: ParentNode = document): T {
  const el = root.querySelector<T>(sel);
  if (!el) throw new Error(`Missing element: ${sel}`);
  return el;
}
const $$ = <T extends Element = HTMLElement>(sel: string, root: ParentNode = document): T[] =>
  [...root.querySelectorAll<T>(sel)];

type Field = HTMLInputElement | HTMLSelectElement;
const field = (form: HTMLFormElement, name: string) => form.elements.namedItem(name) as HTMLInputElement;

const MOCK = __SIMULATOR__ && (new URLSearchParams(location.search).has('mock') || location.protocol === 'file:');
const BREWING = new Set<MachineState>(['preparing', 'pouring', 'waiting', 'paused', 'finishing', 'calibrating']);

interface HistoryPoint {
  t: number;
  w: number;
}

const app: Transport & {
  recipes: Recipe[];
  settings: Settings | null;
  status: Status;
  history: HistoryPoint[];
  editing: number;
  sock: SocketLike | null;
} = {
  recipes: [],
  settings: null,
  status: { t: 'status', state: 'idle', poured: 0, flow: 0 },
  history: [],
  editing: -1,
  sock: null,
  Socket: WebSocket,
  fetch: (url, init) => window.fetch(url, init),
};

// ---------------------------------------------------------------- transport

async function getJson<T>(url: string): Promise<T> {
  const res = await app.fetch(url);
  if (!res.ok) throw new Error(`${url} returned ${res.status}`);
  return res.json();
}

async function postJson(url: string, body: unknown): Promise<unknown> {
  const res = await app.fetch(url, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(body),
  });
  if (!res.ok) {
    const err = (await res.json().catch(() => ({}))) as { error?: string };
    throw new Error(err.error || `Save failed (${res.status})`);
  }
  return res.json();
}

function connect() {
  const url = MOCK ? 'mock' : `${location.protocol === 'https:' ? 'wss' : 'ws'}://${location.host}/ws`;
  const ws = new app.Socket(url);
  ws.onopen = () => setConn('online', 'Connected');
  ws.onclose = () => {
    setConn('offline', 'Reconnecting');
    setTimeout(connect, 1500);
  };
  ws.onmessage = (e) => onMessage(JSON.parse(e.data) as ServerMessage);
  app.sock = ws;
}

function send(command: Command) {
  if (app.sock?.readyState !== WebSocket.OPEN) return toast('Not connected to the machine.', 'error');
  app.sock.send(JSON.stringify(command));
}

function setConn(state: 'online' | 'offline', label: string) {
  const el = $('#conn');
  el.dataset.state = state;
  el.textContent = label;
}

function onMessage(m: ServerMessage) {
  if (m.t === 'status') return renderStatus(m);
  if (m.t === 'error') return toast(m.msg, 'error');
  if (m.t === 'recipes') {
    toast(m.msg);
    return loadRecipes();
  }
  if (m.t === 'info') {
    toast(m.msg);
    loadSettings();
  }
}

// ---------------------------------------------------------------- helpers

let toastTimer: ReturnType<typeof setTimeout> | undefined;
function toast(msg: string, kind: 'info' | 'error' = 'info') {
  const el = $('#toast');
  el.textContent = msg;
  el.dataset.kind = kind;
  el.hidden = false;
  clearTimeout(toastTimer);
  toastTimer = setTimeout(() => (el.hidden = true), kind === 'error' ? 6000 : 3000);
}

const fmt1 = (n: number) => (Math.abs(n) < 0.05 ? 0 : n).toFixed(1);
const clockTime = (s: number) => `${Math.floor(s / 60)}:${String(Math.floor(s % 60)).padStart(2, '0')}`;
const totalWater = (r: Pick<Recipe, 'stages'>) => r.stages.reduce((a, s) => a + (+s.water || 0), 0);
const brewSeconds = (r: Pick<Recipe, 'stages'>) =>
  r.stages.reduce((a, s) => a + (s.water || 0) / (s.flow || 4) + (+s.wait || 0), 0);

function recipeSummary(r: Recipe) {
  const total = totalWater(r);
  const ratio = r.dose ? `, 1:${(total / r.dose).toFixed(1)}` : '';
  return `${r.dose || '?'} g to ${total} g${ratio}, about ${clockTime(brewSeconds(r))}`;
}

function cssVar(name: string) {
  return getComputedStyle(document.documentElement).getPropertyValue(name).trim();
}

function escapeHtml(s: unknown) {
  const map: Record<string, string> = { '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' };
  return String(s).replace(/[&<>"']/g, (c) => map[c]);
}

// ---------------------------------------------------------------- brew view

function activeRecipe(): Recipe | undefined {
  const st = app.status.state;
  const id = BREWING.has(st) || st === 'done' || st === 'error'
    ? app.status.recipe
    : $<HTMLSelectElement>('#recipe-select').value;
  return app.recipes.find((r) => r.id === id) || app.recipes[0];
}

function phaseText(s: Status) {
  const name = s.stageName ? `<strong>${escapeHtml(s.stageName)}</strong>` : '';
  const last = s.stage === (s.stages ?? 0) - 1;
  switch (s.state) {
    case 'preparing': return 'Homing the arm';
    case 'pouring': return `${name}, pouring to ${Math.round(s.target ?? 0)} g`;
    case 'waiting':
      return last ? `${name}, letting it drain` : `${name}, next pour in ${Math.ceil(s.waitLeft ?? 0)} s`;
    case 'paused': return 'Paused';
    case 'finishing': return 'Parking the arm';
    case 'done': return 'Brew finished';
    case 'error': return 'Stopped';
    case 'calibrating': return 'Running a calibration dispense';
    default: return 'Ready';
  }
}

function renderStatus(s: Status) {
  const prev = app.status.state;
  if (s.message && s.message !== app.status.message) {
    toast(s.message);
    loadSettings(); // a calibration finished and may have changed them
  }
  app.status = s;

  if (s.state === 'preparing' && prev !== 'preparing') app.history = [];
  if (BREWING.has(s.state) && s.state !== 'calibrating' && s.elapsed != null) {
    const last = app.history.at(-1);
    if (!last || s.elapsed - last.t >= 0.2) app.history.push({ t: s.elapsed, w: s.poured });
  }

  const recipe = activeRecipe();
  const total = s.total ?? (recipe ? totalWater(recipe) : 0);

  $('#poured').textContent = fmt1(s.poured);
  $('#total').textContent = total ? String(Math.round(total)) : '–';
  $('#flow').textContent = fmt1(Math.max(0, s.flow));
  $('#elapsed').textContent = clockTime(s.elapsed ?? 0);
  $('#phase').innerHTML = phaseText(s);

  const minTemp = s.minTemp ?? recipe?.minTemp ?? 0;
  const tempEl = $('#temp');
  tempEl.textContent = s.temp != null ? `${s.temp.toFixed(1)} °C` : 'No probe';
  tempEl.classList.toggle('warn', s.temp != null && minTemp > 0 && s.temp < minTemp - 0.5);

  renderBeaker(recipe, s, total);
  renderStageStrip(recipe, s);
  drawChart(recipe, total);

  const err = $('#brew-error');
  err.hidden = !s.error;
  err.textContent = s.error || '';

  const brewing = BREWING.has(s.state);
  const finished = s.state === 'done' || s.state === 'error';
  $('.controls').toggleAttribute('data-brewing', brewing);
  $('#btn-start').hidden = brewing;
  $('#btn-start').textContent = finished ? 'Brew again' : 'Start brew';
  $('#btn-pause').hidden = !brewing || s.state === 'finishing' || s.state === 'calibrating';
  $('#btn-pause').textContent = s.state === 'paused' ? 'Resume' : 'Pause';
  $('#btn-stop').hidden = !(brewing || finished);
  $('#btn-stop').textContent = finished ? 'Dismiss' : 'Stop';
  $<HTMLSelectElement>('#recipe-select').disabled = brewing;

  $('#meter-poured').textContent = fmt1(s.poured);
  $('#motion-state').textContent = s.homed ? (s.motion ?? '') : `${s.motion}, not homed`;
}

// The full recipe fills the beaker to this height, leaving headroom for the top graduation.
const BEAKER_FULL = 90;

function renderBeaker(recipe: Recipe | undefined, s: Status, total: number) {
  const fill = $('#beaker-fill');
  fill.style.height = total ? `${Math.min(100, Math.max(0, (s.poured / total) * BEAKER_FULL))}%` : '0%';

  const ticks = $('#beaker-ticks');
  const key = recipe ? recipe.id + JSON.stringify(recipe.stages.map((x) => x.water)) : '';
  if (ticks.dataset.key !== key) {
    ticks.dataset.key = key;
    ticks.innerHTML = '';
    let cum = 0;
    for (const st of recipe?.stages ?? []) {
      cum += +st.water || 0;
      if (!st.water) continue;
      const t = document.createElement('div');
      t.className = 'tick';
      t.style.bottom = `${(cum / total) * BEAKER_FULL}%`;
      t.innerHTML = `<span>${Math.round(cum)}</span>`;
      t.dataset.cum = String(cum);
      ticks.append(t);
    }
  }
  const active = BREWING.has(s.state) && s.target != null;
  for (const t of $$('.tick', ticks)) {
    const cum = +(t.dataset.cum ?? 0);
    t.toggleAttribute('data-current', active && Math.abs(cum - (s.target ?? 0)) < 0.01);
    t.toggleAttribute('data-done', s.poured >= cum - 0.5 && cum <= (s.target ?? 0) + 0.01);
  }
}

function renderStageStrip(recipe: Recipe | undefined, s: Status) {
  const ol = $('#stages');
  const stages = recipe?.stages ?? [];
  if (ol.children.length !== stages.length || ol.dataset.recipe !== recipe?.id) {
    ol.dataset.recipe = recipe?.id ?? '';
    ol.innerHTML = stages
      .map((st) => `<li><b>${escapeHtml(st.name)}</b>${st.water} g${st.wait ? `, ${st.wait} s` : ''}</li>`)
      .join('');
  }
  const running = BREWING.has(s.state) || s.state === 'done';
  $$('li', ol).forEach((li, i) => {
    let st = '';
    if (running && s.stage != null) st = i < s.stage || s.state === 'done' ? 'done' : i === s.stage ? 'current' : '';
    li.dataset.state = st;
  });
}

function drawChart(recipe: Recipe | undefined, total: number) {
  const c = $<HTMLCanvasElement>('#chart');
  const dpr = window.devicePixelRatio || 1;
  const w = c.clientWidth;
  const h = c.clientHeight;
  if (c.width !== w * dpr) c.width = w * dpr;
  if (c.height !== h * dpr) c.height = h * dpr;
  const g = c.getContext('2d');
  if (!g) return;
  g.setTransform(dpr, 0, 0, dpr, 0, 0);
  g.clearRect(0, 0, w, h);

  const span = Math.max(60, recipe ? brewSeconds(recipe) * 1.1 : 60, app.history.at(-1)?.t ?? 0);
  const top = Math.max(total, 1) * 1.05;
  const x = (t: number) => (t / span) * (w - 2) + 1;
  const y = (v: number) => h - 14 - (v / top) * (h - 20);

  g.font = `11px ${cssVar('--font')}`;
  g.fillStyle = cssVar('--ink-soft');
  g.strokeStyle = cssVar('--line');
  g.lineWidth = 1;
  for (let t = 0; t <= span; t += 60) {
    g.beginPath();
    g.moveTo(x(t), h - 14);
    g.lineTo(x(t), h - 8);
    g.stroke();
    g.fillText(`${t / 60}m`, Math.min(x(t) + 3, w - 18), h - 1);
  }
  let cum = 0;
  g.setLineDash([3, 4]);
  for (const st of recipe?.stages ?? []) {
    cum += +st.water || 0;
    g.beginPath();
    g.moveTo(0, y(cum));
    g.lineTo(w, y(cum));
    g.stroke();
  }
  g.setLineDash([]);

  if (app.history.length > 1) {
    g.strokeStyle = cssVar('--water');
    g.lineWidth = 2.5;
    g.lineJoin = 'round';
    g.beginPath();
    app.history.forEach((p, i) => (i ? g.lineTo(x(p.t), y(p.w)) : g.moveTo(x(p.t), y(p.w))));
    g.stroke();
  }
}

function renderRecipeSelect() {
  const sel = $<HTMLSelectElement>('#recipe-select');
  const keep = sel.value || app.settings?.lastRecipe;
  sel.innerHTML = app.recipes
    .map((r) => `<option value="${escapeHtml(r.id)}">${escapeHtml(r.name)}, ${totalWater(r)} g</option>`)
    .join('');
  if (keep && app.recipes.some((r) => r.id === keep)) sel.value = keep;
  renderStatus(app.status);
}

// ---------------------------------------------------------------- recipes view

function renderRecipeList() {
  $('#recipe-list').innerHTML = app.recipes.length
    ? app.recipes
        .map((r, i) => `<li><button data-edit="${i}"><span>${escapeHtml(r.name)}</span>
           <span class="meta">${r.dose || '?'} g : ${totalWater(r)} g</span></button></li>`)
        .join('')
    : '<li class="hint">No recipes yet. Create one to start brewing.</li>';
}

function stageEditor(st: Partial<Stage>) {
  const li = $<HTMLTemplateElement>('#tpl-stage').content.firstElementChild!.cloneNode(true) as HTMLElement;
  for (const [k, v] of Object.entries(st)) {
    const input = li.querySelector<Field>(`[name="${k}"]`);
    if (input) input.value = String(v);
  }
  return li;
}

const NEW_RECIPE: Omit<Recipe, 'id'> = {
  name: 'New recipe', dose: 15, minTemp: 92,
  stages: [{ name: 'Bloom', water: 45, flow: 4, pattern: 'spiral', radius: 22, rps: 1, wait: 40 }],
};

function openEditor(index: number) {
  app.editing = index;
  const r = index >= 0 ? app.recipes[index] : NEW_RECIPE;
  const form = $<HTMLFormElement>('#recipe-editor');
  field(form, 'recipeName').value = r.name;
  field(form, 'dose').value = String(r.dose ?? '');
  field(form, 'minTemp').value = String(r.minTemp ?? '');
  $('#stage-editors').replaceChildren(...r.stages.map(stageEditor));
  $('#btn-delete').hidden = index < 0;
  $('#btn-duplicate').hidden = index < 0;
  $('#recipe-list-pane').hidden = true;
  form.hidden = false;
  updateEditorSummary();
  window.scrollTo({ top: 0 });
}

function closeEditor() {
  $('#recipe-editor').hidden = true;
  $('#recipe-list-pane').hidden = false;
}

function readEditor(): Recipe {
  const form = $<HTMLFormElement>('#recipe-editor');
  const num = (v: string, d = 0) => (v === '' || isNaN(+v) ? d : +v);
  const stages = $$('.stage-editor', form).map((li): Stage => {
    const f = (n: string) => $<Field>(`[name="${n}"]`, li).value;
    return {
      name: f('name').trim() || 'Pour',
      water: num(f('water')),
      flow: num(f('flow'), 4),
      pattern: f('pattern') as Pattern,
      radius: num(f('radius'), 20),
      rps: num(f('rps'), 1),
      wait: num(f('wait')),
    };
  });
  const existing = app.recipes[app.editing];
  const name = field(form, 'recipeName').value.trim() || 'Untitled';
  return {
    id: existing?.id ?? `${name.toLowerCase().replace(/[^a-z0-9]+/g, '-').replace(/^-|-$/g, '')}-${Date.now().toString(36)}`,
    name,
    dose: num(field(form, 'dose').value),
    minTemp: num(field(form, 'minTemp').value),
    stages,
  };
}

function updateEditorSummary() {
  const r = readEditor();
  $('#editor-summary').textContent = r.stages.length ? recipeSummary(r) : 'Add at least one stage.';
}

async function saveRecipes(list: Recipe[], okMsg?: string) {
  try {
    await postJson('/api/recipes', list);
    app.recipes = list;
    renderRecipeList();
    renderRecipeSelect();
    if (okMsg) toast(okMsg);
    return true;
  } catch (e) {
    toast((e as Error).message, 'error');
    return false;
  }
}

// ---------------------------------------------------------------- machine view

const ADVANCED_FIELDS: Partial<Record<keyof Settings, string>> = {
  flowPulsesPerLitre: 'Flow meter pulses per litre',
  pumpGpsAtFull: 'Pump rate at full power (g/s)',
  pumpMinDuty: 'Pump minimum power (0–1)',
  pumpLagS: 'Pour stop lead time (s)',
  thetaStepsPerDeg: 'Arm steps per degree',
  radialStepsPerMm: 'Carriage steps per mm',
  invertTheta: 'Reverse arm direction',
  invertRadial: 'Reverse carriage direction',
  thetaHomeDeg: 'Arm angle at endstop (°)',
  radialHomeMm: 'Carriage radius at endstop (mm)',
  thetaMinDeg: 'Arm minimum angle (°)',
  thetaMaxDeg: 'Arm maximum angle (°)',
  radialMinMm: 'Carriage minimum radius (mm)',
  radialMaxMm: 'Carriage maximum radius (mm)',
  centerR: 'Dripper centre radius (mm)',
  centerThetaDeg: 'Dripper centre angle (°)',
  parkR: 'Park radius (mm)',
  parkThetaDeg: 'Park angle (°)',
  hostname: 'Network name (.local)',
};

function renderSettings() {
  const s = app.settings;
  if (!s) return;
  $('#pump-rate').textContent = s.pumpGpsAtFull?.toFixed(2) ?? '–';
  $('#meter-ppl').textContent = s.flowPulsesPerLitre?.toFixed(0) ?? '–';
  $('#wifi-mode').textContent = s.apMode
    ? 'The machine is running its own access point. Join your home network so you can reach it at ' +
      `${s.hostname}.local from any device.`
    : `Connected to ${s.wifiSsid}. Open ${s.hostname}.local from any device on this network.`;
  field($<HTMLFormElement>('#wifi-form'), 'wifiSsid').value = s.wifiSsid || '';

  const form = $('#advanced-form');
  form.innerHTML = (Object.entries(ADVANCED_FIELDS) as [keyof Settings, string][])
    .filter(([k]) => k in s)
    .map(([k, label]) => {
      const v = s[k];
      return typeof v === 'boolean'
        ? `<label class="check"><input type="checkbox" name="${k}" ${v ? 'checked' : ''}>${label}</label>`
        : `<label class="field">${label}<input name="${k}" value="${escapeHtml(v)}"
             ${typeof v === 'number' ? 'type="number" step="any" inputmode="decimal"' : ''}></label>`;
    })
    .join('') + '<button type="submit" class="primary wide">Save settings</button>';
}

async function loadSettings() {
  try {
    app.settings = await getJson<Settings>('/api/settings');
    renderSettings();
  } catch {
    /* shown via connection state */
  }
}

async function loadRecipes() {
  try {
    app.recipes = await getJson<Recipe[]>('/api/recipes');
  } catch {
    app.recipes = [];
  }
  renderRecipeList();
  renderRecipeSelect();
}

// ---------------------------------------------------------------- wiring

function showView(name: string) {
  for (const v of $$('.view')) v.toggleAttribute('data-active', v.id === `view-${name}`);
  for (const b of $$('.tabs button')) {
    if (b.dataset.view === name) b.setAttribute('aria-current', 'page');
    else b.removeAttribute('aria-current');
  }
  if (name === 'brew') requestAnimationFrame(() => drawChart(activeRecipe(), app.status.total ?? 0));
}

function bind() {
  $$('.tabs button').forEach((b) => b.addEventListener('click', () => showView(b.dataset.view ?? 'brew')));

  $('#btn-start').addEventListener('click', () => {
    const id = $<HTMLSelectElement>('#recipe-select').value;
    if (!id) return toast('Create a recipe first.', 'error');
    send({ cmd: 'start', recipe: id });
  });
  $('#btn-pause').addEventListener('click', () => send({ cmd: app.status.state === 'paused' ? 'resume' : 'pause' }));
  $('#btn-stop').addEventListener('click', () => send({ cmd: 'stop' }));
  $('#recipe-select').addEventListener('change', () => renderStatus(app.status));

  $('#recipe-list').addEventListener('click', (e) => {
    const b = (e.target as Element).closest<HTMLElement>('[data-edit]');
    if (b) openEditor(+(b.dataset.edit ?? -1));
  });
  $('#btn-new-recipe').addEventListener('click', () => openEditor(-1));
  $('#btn-editor-back').addEventListener('click', closeEditor);
  $('#btn-add-stage').addEventListener('click', () => {
    const prev = readEditor().stages.at(-1);
    $('#stage-editors').append(stageEditor({
      ...(prev ?? { flow: 4, pattern: 'spiral', radius: 22, rps: 1 }),
      name: `Pour ${$$('.stage-editor').length}`,
      water: 50,
      wait: 10,
    }));
    updateEditorSummary();
  });
  $('#stage-editors').addEventListener('click', (e) => {
    const target = e.target as Element;
    const act = target.closest<HTMLElement>('[data-act]')?.dataset.act;
    const li = target.closest('.stage-editor');
    if (!act || !li) return;
    if (act === 'remove') li.remove();
    if (act === 'up' && li.previousElementSibling) li.previousElementSibling.before(li);
    if (act === 'down' && li.nextElementSibling) li.nextElementSibling.after(li);
    updateEditorSummary();
  });
  $('#recipe-editor').addEventListener('input', updateEditorSummary);
  $('#recipe-editor').addEventListener('submit', async (e) => {
    e.preventDefault();
    const r = readEditor();
    if (!r.stages.length) return toast('Add at least one stage.', 'error');
    const list = [...app.recipes];
    if (app.editing >= 0) list[app.editing] = r;
    else list.push(r);
    if (await saveRecipes(list)) closeEditor();
  });
  $('#btn-duplicate').addEventListener('click', async () => {
    const copy = { ...readEditor(), name: `${readEditor().name} copy` };
    copy.id = `${copy.id.replace(/-[a-z0-9]+$/, '')}-${Date.now().toString(36)}`;
    if (await saveRecipes([...app.recipes, copy])) openEditor(app.recipes.length - 1);
  });
  $('#btn-delete').addEventListener('click', async () => {
    const r = app.recipes[app.editing];
    if (!r || !confirm(`Delete “${r.name}”?`)) return;
    if (await saveRecipes(app.recipes.filter((_, i) => i !== app.editing), 'Recipe deleted.')) closeEditor();
  });

  $$('[data-cmd]').forEach((b) =>
    b.addEventListener('click', () => {
      const cmd = b.dataset.cmd;
      if (cmd === 'prime') send({ cmd: 'prime', seconds: 3, duty: 1 });
      else send({ cmd } as Command);
    }));
  $$('[data-jog]').forEach((b) =>
    b.addEventListener('click', () => {
      const [axis, dir] = (b.dataset.jog ?? '').split(':');
      const mm = +$<HTMLSelectElement>('#jog-step').value * +dir;
      if (axis === 'r') send({ cmd: 'jog', dr: mm, dtheta: 0 });
      else send({ cmd: 'jog', dr: 0, dtheta: (mm / (app.settings?.centerR || 110)) * (180 / Math.PI) });
    }));
  $('#btn-cal-meter').addEventListener('click', () => {
    const ml = +$<HTMLInputElement>('#cal-ml').value;
    if (!(ml >= 10)) return toast('Enter how much water came out, in ml.', 'error');
    send({ cmd: 'calMeter', ml });
  });
  $('#wifi-form').addEventListener('submit', async (e) => {
    e.preventDefault();
    const f = e.target as HTMLFormElement;
    try {
      await postJson('/api/settings', { wifiSsid: field(f, 'wifiSsid').value.trim(), wifiPass: field(f, 'wifiPass').value });
    } catch (err) {
      toast((err as Error).message, 'error');
    }
  });
  $('#advanced-form').addEventListener('submit', async (e) => {
    e.preventDefault();
    const out: Record<string, string | number | boolean> = {};
    for (const el of (e.target as HTMLFormElement).elements as HTMLFormControlsCollection & Iterable<HTMLInputElement>) {
      if (!el.name) continue;
      const orig = app.settings?.[el.name as keyof Settings];
      out[el.name] = el.type === 'checkbox' ? el.checked : typeof orig === 'number' ? +el.value : el.value;
    }
    try {
      await postJson('/api/settings', out);
    } catch (err) {
      toast((err as Error).message, 'error');
    }
  });

  window.addEventListener('resize', () => drawChart(activeRecipe(), app.status.total ?? 0));
}

async function main() {
  // The literal __SIMULATOR__ check lets esbuild drop the simulator from firmware builds.
  if (__SIMULATOR__ && MOCK) {
    const { createMock } = await import('./mock');
    Object.assign(app, createMock());
    document.title = 'OpenPour (simulator)';
  }
  bind();
  await loadSettings();
  await loadRecipes();
  connect();
}

main();
