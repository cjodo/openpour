// OpenPour control app. Plain ES modules, no build step: edit and reload.
// Open with ?mock to run against a simulated machine (see mock.js).

const $ = (sel, el = document) => el.querySelector(sel);
const $$ = (sel, el = document) => [...el.querySelectorAll(sel)];

const MOCK = new URLSearchParams(location.search).has('mock') || location.protocol === 'file:';
const BREWING = new Set(['preparing', 'pouring', 'waiting', 'paused', 'finishing', 'calibrating']);

const app = {
  recipes: [],
  settings: null,
  status: { state: 'idle', poured: 0, flow: 0 },
  history: [],
  editing: -1,
  sock: null,
  Socket: WebSocket,
  fetch: (...a) => window.fetch(...a),
};

// ---------------------------------------------------------------- transport

async function getJson(url) {
  const res = await app.fetch(url);
  if (!res.ok) throw new Error(`${url} returned ${res.status}`);
  return res.json();
}

async function postJson(url, body) {
  const res = await app.fetch(url, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(body),
  });
  if (!res.ok) throw new Error((await res.json().catch(() => ({}))).error || `Save failed (${res.status})`);
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
  ws.onmessage = (e) => onMessage(JSON.parse(e.data));
  app.sock = ws;
}

function send(cmd, extra = {}) {
  if (app.sock?.readyState !== 1) return toast('Not connected to the machine.', 'error');
  app.sock.send(JSON.stringify({ cmd, ...extra }));
}

function setConn(state, label) {
  const el = $('#conn');
  el.dataset.state = state;
  el.textContent = label;
}

function onMessage(m) {
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

let toastTimer;
function toast(msg, kind = 'info') {
  const el = $('#toast');
  el.textContent = msg;
  el.dataset.kind = kind;
  el.hidden = false;
  clearTimeout(toastTimer);
  toastTimer = setTimeout(() => (el.hidden = true), kind === 'error' ? 6000 : 3000);
}

const fmt1 = (n) => (Math.abs(n) < 0.05 ? 0 : n).toFixed(1);
const clockTime = (s) => `${Math.floor(s / 60)}:${String(Math.floor(s % 60)).padStart(2, '0')}`;
const totalWater = (r) => r.stages.reduce((a, s) => a + (+s.water || 0), 0);
const brewSeconds = (r) => r.stages.reduce((a, s) => a + (s.water || 0) / (s.flow || 4) + (+s.wait || 0), 0);

function recipeSummary(r) {
  const total = totalWater(r);
  const ratio = r.dose ? `, 1:${(total / r.dose).toFixed(1)}` : '';
  return `${r.dose || '?'} g to ${total} g${ratio}, about ${clockTime(brewSeconds(r))}`;
}

function cssVar(name) {
  return getComputedStyle(document.documentElement).getPropertyValue(name).trim();
}

// ---------------------------------------------------------------- brew view

function activeRecipe() {
  const id = BREWING.has(app.status.state) || app.status.state === 'done' || app.status.state === 'error'
    ? app.status.recipe
    : $('#recipe-select').value;
  return app.recipes.find((r) => r.id === id) || app.recipes[0];
}

function phaseText(s) {
  const name = s.stageName ? `<strong>${escapeHtml(s.stageName)}</strong>` : '';
  const last = s.stage === s.stages - 1;
  switch (s.state) {
    case 'preparing': return 'Homing the arm';
    case 'pouring': return `${name}, pouring to ${Math.round(s.target)} g`;
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

function escapeHtml(s) {
  return String(s).replace(/[&<>"']/g, (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' })[c]);
}

function renderStatus(s) {
  const prev = app.status.state;
  if (s.message && s.message !== app.status.message) {
    toast(s.message);
    loadSettings(); // a calibration finished and may have changed them
  }
  app.status = s;

  if (s.state === 'preparing' && prev !== 'preparing') app.history = [];
  if (BREWING.has(s.state) && s.state !== 'calibrating' && s.elapsed != null) {
    const last = app.history[app.history.length - 1];
    if (!last || s.elapsed - last.t >= 0.2) app.history.push({ t: s.elapsed, w: s.poured });
  }

  const recipe = activeRecipe();
  const total = s.total ?? (recipe ? totalWater(recipe) : 0);

  $('#poured').textContent = fmt1(s.poured);
  $('#total').textContent = total ? Math.round(total) : '–';
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
  $('#recipe-select').disabled = brewing;

  $('#meter-poured').textContent = fmt1(s.poured);
  $('#motion-state').textContent = s.homed ? s.motion : `${s.motion}, not homed`;
}

// The full recipe fills the beaker to this height, leaving headroom for the top graduation.
const BEAKER_FULL = 90;

function renderBeaker(recipe, s, total) {
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
      t.dataset.cum = cum;
      ticks.append(t);
    }
  }
  const active = BREWING.has(s.state) && s.target != null;
  for (const t of ticks.children) {
    const cum = +t.dataset.cum;
    t.toggleAttribute('data-current', active && Math.abs(cum - s.target) < 0.01);
    t.toggleAttribute('data-done', s.poured >= cum - 0.5 && cum <= (s.target ?? 0) + 0.01);
  }
}

function renderStageStrip(recipe, s) {
  const ol = $('#stages');
  const stages = recipe?.stages ?? [];
  if (ol.children.length !== stages.length || ol.dataset.recipe !== recipe?.id) {
    ol.dataset.recipe = recipe?.id ?? '';
    ol.innerHTML = stages
      .map((st) => `<li><b>${escapeHtml(st.name)}</b>${st.water} g${st.wait ? `, ${st.wait} s` : ''}</li>`)
      .join('');
  }
  const running = BREWING.has(s.state) || s.state === 'done';
  [...ol.children].forEach((li, i) => {
    let st = '';
    if (running && s.stage != null) st = i < s.stage || s.state === 'done' ? 'done' : i === s.stage ? 'current' : '';
    li.dataset.state = st;
  });
}

function drawChart(recipe, total) {
  const c = $('#chart');
  const dpr = window.devicePixelRatio || 1;
  const w = c.clientWidth;
  const h = c.clientHeight;
  if (c.width !== w * dpr) c.width = w * dpr;
  if (c.height !== h * dpr) c.height = h * dpr;
  const g = c.getContext('2d');
  g.setTransform(dpr, 0, 0, dpr, 0, 0);
  g.clearRect(0, 0, w, h);

  const span = Math.max(60, recipe ? brewSeconds(recipe) * 1.1 : 60, app.history.at(-1)?.t ?? 0);
  const top = Math.max(total, 1) * 1.05;
  const x = (t) => (t / span) * (w - 2) + 1;
  const y = (v) => h - 14 - (v / top) * (h - 20);

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
  const sel = $('#recipe-select');
  const keep = sel.value || app.settings?.lastRecipe;
  sel.innerHTML = app.recipes
    .map((r) => `<option value="${escapeHtml(r.id)}">${escapeHtml(r.name)}, ${totalWater(r)} g</option>`)
    .join('');
  if (app.recipes.some((r) => r.id === keep)) sel.value = keep;
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

function stageEditor(st) {
  const li = $('#tpl-stage').content.firstElementChild.cloneNode(true);
  for (const [k, v] of Object.entries(st)) {
    const input = li.querySelector(`[name="${k}"]`);
    if (input) input.value = v;
  }
  return li;
}

function openEditor(index) {
  app.editing = index;
  const r = index >= 0 ? app.recipes[index] : {
    name: 'New recipe', dose: 15, minTemp: 92,
    stages: [{ name: 'Bloom', water: 45, flow: 4, pattern: 'spiral', radius: 22, rps: 1, wait: 40 }],
  };
  const form = $('#recipe-editor');
  form.recipeName.value = r.name;
  form.dose.value = r.dose ?? '';
  form.minTemp.value = r.minTemp ?? '';
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

function readEditor() {
  const form = $('#recipe-editor');
  const num = (v, d = 0) => (v === '' || isNaN(+v) ? d : +v);
  const stages = $$('.stage-editor', form).map((li) => {
    const f = (n) => li.querySelector(`[name="${n}"]`).value;
    return {
      name: f('name').trim() || 'Pour',
      water: num(f('water')),
      flow: num(f('flow'), 4),
      pattern: f('pattern'),
      radius: num(f('radius'), 20),
      rps: num(f('rps'), 1),
      wait: num(f('wait')),
    };
  });
  const existing = app.recipes[app.editing];
  const name = form.recipeName.value.trim() || 'Untitled';
  return {
    id: existing?.id ?? `${name.toLowerCase().replace(/[^a-z0-9]+/g, '-').replace(/^-|-$/g, '')}-${Date.now().toString(36)}`,
    name,
    dose: num(form.dose.value),
    minTemp: num(form.minTemp.value),
    stages,
  };
}

function updateEditorSummary() {
  const r = readEditor();
  $('#editor-summary').textContent = r.stages.length ? recipeSummary(r) : 'Add at least one stage.';
}

async function saveRecipes(list, okMsg) {
  try {
    await postJson('/api/recipes', list);
    app.recipes = list;
    renderRecipeList();
    renderRecipeSelect();
    if (okMsg) toast(okMsg);
    return true;
  } catch (e) {
    toast(e.message, 'error');
    return false;
  }
}

// ---------------------------------------------------------------- machine view

const ADVANCED_FIELDS = {
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
  $('#wifi-form').wifiSsid.value = s.wifiSsid || '';

  const form = $('#advanced-form');
  form.innerHTML = Object.entries(ADVANCED_FIELDS)
    .filter(([k]) => k in s)
    .map(([k, label]) =>
      typeof s[k] === 'boolean'
        ? `<label class="check"><input type="checkbox" name="${k}" ${s[k] ? 'checked' : ''}>${label}</label>`
        : `<label class="field">${label}<input name="${k}" value="${escapeHtml(s[k])}"
             ${typeof s[k] === 'number' ? 'type="number" step="any" inputmode="decimal"' : ''}></label>`)
    .join('') + '<button type="submit" class="primary wide">Save settings</button>';
}

async function loadSettings() {
  try {
    app.settings = await getJson('/api/settings');
    renderSettings();
  } catch {
    /* shown via connection state */
  }
}

async function loadRecipes() {
  try {
    app.recipes = await getJson('/api/recipes');
  } catch {
    app.recipes = [];
  }
  renderRecipeList();
  renderRecipeSelect();
}

// ---------------------------------------------------------------- wiring

function showView(name) {
  for (const v of $$('.view')) v.toggleAttribute('data-active', v.id === `view-${name}`);
  for (const b of $$('.tabs button')) {
    if (b.dataset.view === name) b.setAttribute('aria-current', 'page');
    else b.removeAttribute('aria-current');
  }
  if (name === 'brew') requestAnimationFrame(() => drawChart(activeRecipe(), app.status.total ?? 0));
}

function bind() {
  $$('.tabs button').forEach((b) => b.addEventListener('click', () => showView(b.dataset.view)));

  $('#btn-start').addEventListener('click', () => {
    const id = $('#recipe-select').value;
    if (!id) return toast('Create a recipe first.', 'error');
    send('start', { recipe: id });
  });
  $('#btn-pause').addEventListener('click', () => send(app.status.state === 'paused' ? 'resume' : 'pause'));
  $('#btn-stop').addEventListener('click', () => send('stop'));
  $('#recipe-select').addEventListener('change', () => renderStatus(app.status));

  $('#recipe-list').addEventListener('click', (e) => {
    const b = e.target.closest('[data-edit]');
    if (b) openEditor(+b.dataset.edit);
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
    const act = e.target.closest('[data-act]')?.dataset.act;
    if (!act) return;
    const li = e.target.closest('.stage-editor');
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
      if (cmd === 'prime') send('prime', { seconds: 3, duty: 1 });
      else send(cmd);
    }));
  $$('[data-jog]').forEach((b) =>
    b.addEventListener('click', () => {
      const [axis, dir] = b.dataset.jog.split(':');
      const mm = +$('#jog-step').value * +dir;
      if (axis === 'r') send('jog', { dr: mm, dtheta: 0 });
      else send('jog', { dr: 0, dtheta: (mm / (app.settings?.centerR || 110)) * (180 / Math.PI) });
    }));
  $('#btn-cal-meter').addEventListener('click', () => {
    const ml = +$('#cal-ml').value;
    if (!(ml >= 10)) return toast('Enter how much water came out, in ml.', 'error');
    send('calMeter', { ml });
  });
  $('#wifi-form').addEventListener('submit', async (e) => {
    e.preventDefault();
    const f = e.target;
    try {
      await postJson('/api/settings', { wifiSsid: f.wifiSsid.value.trim(), wifiPass: f.wifiPass.value });
    } catch (err) {
      toast(err.message, 'error');
    }
  });
  $('#advanced-form').addEventListener('submit', async (e) => {
    e.preventDefault();
    const out = {};
    for (const el of e.target.elements) {
      if (!el.name) continue;
      const orig = app.settings[el.name];
      out[el.name] = el.type === 'checkbox' ? el.checked : typeof orig === 'number' ? +el.value : el.value;
    }
    try {
      await postJson('/api/settings', out);
    } catch (err) {
      toast(err.message, 'error');
    }
  });

  window.addEventListener('resize', () => drawChart(activeRecipe(), app.status.total ?? 0));
}

async function main() {
  if (MOCK) {
    const mock = await import('./mock.js');
    Object.assign(app, mock.createMock());
    document.title = 'OpenPour (simulator)';
  }
  bind();
  await loadSettings();
  await loadRecipes();
  connect();
}

main();
