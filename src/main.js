import "./style.css";
import { Rope, reducedMotion, svgPoint } from "./rope.js";

const NS = "http://www.w3.org/2000/svg";
const el = (tag, attrs = {}) => {
  const e = document.createElementNS(NS, tag);
  for (const [k, v] of Object.entries(attrs)) e.setAttribute(k, v);
  return e;
};
const RM = reducedMotion();
const CABLE = ["#e8b628", "#d9534a", "#4a90d9", "#5cb85c", "#c27bd6", "#e8893a"];

// ── One animation loop for every scene. A scene says whether it still needs frames.
const scenes = new Set();
let raf = 0, last = 0;
function frame(t) {
  const dt = Math.min(50, t - (last || t));
  last = t;
  let busy = false;
  for (const s of scenes) if (s.visible && s.tick(t, dt)) busy = true;
  raf = busy ? requestAnimationFrame(frame) : 0;
  if (!busy) last = 0;
}
const wake = () => { if (!raf) raf = requestAnimationFrame(frame); };
function addScene(scene, node) {
  scene.visible = false;
  scenes.add(scene);
  new IntersectionObserver(([e]) => { scene.visible = e.isIntersecting; if (scene.visible) wake(); }).observe(node);
}

// ── Module rack: the 24 module kinds from kabl's README, grouped by family.
const families = [
  ["Oscillators", ["Analog-style", "FM, two-operator", "FM, six-operator", "Wavetable"]],
  ["Filters", ["State-variable", "Ladder"]],
  ["Modulation", ["ADSR", "LFO", "Noise"]],
  ["Time", ["Clock", "Clock divider", "Step sequencer"]],
  ["Level & mix", ["VCA", "Gain", "Mixer", "Ring modulator"]],
  ["Effects", ["Drive", "Chorus", "Delay", "Reverb"]],
  ["Control & I/O", ["Macros", "Cues", "MIDI in", "Output"]],
];
{
  const rack = document.getElementById("rack");
  let n = 0;
  for (const [family, kinds] of families) {
    const g = document.createElement("div");
    g.className = "family";
    g.innerHTML = `<h3 class="mono">${family}</h3><ul>${kinds
      .map((k) => `<li><span class="led" aria-hidden="true"></span><span class="idx mono">${String(++n).padStart(2, "0")}</span>${k}</li>`)
      .join("")}</ul>`;
    rack.append(g);
  }
}

// ── Hero: real cables hanging between jacks. Grab a plug, drop it on another jack.
{
  const host = document.getElementById("hero-cable");
  const hero = host.closest(".hero");
  const svg = el("svg");
  host.append(svg);
  const jacksG = el("g"), cablesG = el("g");
  svg.append(jacksG, cablesG);
  // Jack layout in unit coordinates; cables start at fixed sources.
  const jackUnits = [[0.12, 0.2], [0.55, 0.14], [0.9, 0.3], [0.3, 0.7], [0.72, 0.82], [0.95, 0.68]];
  const cables = [
    { from: 0, to: 4, color: CABLE[0] },
    { from: 1, to: 5, color: CABLE[1] },
    { from: 3, to: 2, color: CABLE[2] },
  ];
  let W = 0, H = 0, jacks = [];
  for (const c of cables) {
    c.rope = new Rope(22, 1.12, 40);
    c.g = el("g", { class: "hc", style: `--c:${c.color}` });
    c.sheath = el("path", { class: "sheath" });
    c.core = el("path", { class: "core" });
    c.plug = el("circle", { class: "plug", r: 11 });
    c.g.append(c.sheath, c.core, c.plug);
    cablesG.append(c.g);
  }
  const layout = () => {
    W = host.clientWidth; H = host.clientHeight;
    if (!W || !H) return;
    svg.setAttribute("viewBox", `0 0 ${W} ${H}`);
    jacks = jackUnits.map(([x, y]) => ({ x: 24 + x * (W - 48), y: 20 + y * (H - 40) }));
    jacksG.replaceChildren(...jacks.map((j) => el("circle", { class: "jack-s", cx: j.x, cy: j.y, r: 9 })));
    for (const c of cables) {
      c.end = { ...jacks[c.to] };
      c.rope.reset(jacks[c.from], c.end);
      c.rope.settle(jacks[c.from], c.end);
    }
    draw();
  };
  const draw = () => {
    for (const c of cables) {
      const d = c.rope.path();
      c.sheath.setAttribute("d", d);
      c.core.setAttribute("d", d);
      c.plug.setAttribute("cx", c.end.x);
      c.plug.setAttribute("cy", c.end.y);
    }
  };
  let drag = null, pointer = null;
  const occupied = (j, except) => cables.some((c) => c !== except && (c.from === j || c.to === j));
  svg.addEventListener("pointerdown", (e) => {
    const c = cables.find((c) => c.plug === e.target);
    if (!c) return;
    drag = c;
    c.g.classList.add("held");
    svg.setPointerCapture(e.pointerId);
    e.preventDefault();
  });
  hero.addEventListener("pointermove", (e) => {
    if (!W) return;
    const p = svgPoint(svg, e);
    if (drag) {
      drag.end.x = Math.max(0, Math.min(W, p.x));
      drag.end.y = Math.max(0, Math.min(H, p.y));
    } else if (pointer && !RM) {
      // Brushing past a cable nudges it.
      const vx = p.x - pointer.x, vy = p.y - pointer.y;
      for (const c of cables) for (const q of c.rope.p) {
        const d = Math.hypot(q.x - p.x, q.y - p.y);
        if (d < 36) { q.x += vx * 0.35 * (1 - d / 36); q.y += vy * 0.35 * (1 - d / 36); }
      }
    }
    pointer = p;
    if (RM) { for (const c of cables) c.rope.settle(jacks[c.from], c.end, 30); draw(); }
    wake();
  });
  hero.addEventListener("pointerleave", () => { pointer = null; });
  const drop = () => {
    if (!drag) return;
    const c = drag;
    drag = null;
    c.g.classList.remove("held");
    // Snap to the nearest free jack (or back where it came from).
    let best = c.to, bd = Infinity;
    jacks.forEach((j, i) => {
      if (i === c.from || occupied(i, c)) return;
      const d = Math.hypot(j.x - c.end.x, j.y - c.end.y);
      if (d < bd) { bd = d; best = i; }
    });
    c.to = best;
    c.home = jacks[best];
    if (RM) { c.end = { ...c.home }; c.rope.settle(jacks[c.from], c.end, 60); draw(); }
    wake();
  };
  svg.addEventListener("pointerup", drop);
  svg.addEventListener("pointercancel", drop);

  if (!RM) {
    addScene({
      tick(t) {
        if (!W) return false;
        const wind = Math.sin(t / 1300) * 0.035 + Math.sin(t / 470) * 0.012;
        for (const c of cables) {
          if (c.home && c !== drag) {
            c.end.x += (c.home.x - c.end.x) * 0.25;
            c.end.y += (c.home.y - c.end.y) * 0.25;
            if (Math.hypot(c.home.x - c.end.x, c.home.y - c.end.y) < 0.5) { c.end = { ...c.home }; c.home = null; }
          }
          c.rope.step(jacks[c.from], c.end, { wind, gravity: 0.45 });
        }
        draw();
        return true; // sways gently while on screen
      },
    }, host);
  }
  new ResizeObserver(layout).observe(host);
}

// ── Crate graph: workspace crates and their direct dependencies (from each crate's
// Cargo.toml, transitive edges removed). Descriptions from the README. Drag the crates.
{
  const crates = {
    core: "Op log, patch state, file format. No audio dependencies.",
    modules: "The module kinds.",
    cables: "Functional cables: pattern, probability, morph, glide.",
    engine: "Graph compiler, scheduler, voices.",
    standalone: "Audio and MIDI I/O.",
    ui: "The egui rack and sound browser; the kabl-ui app.",
    clap: "The CLAP instrument plugin, kabl.clap.",
  };
  const edges = [["core", "engine"], ["modules", "engine"], ["cables", "engine"], ["engine", "standalone"], ["standalone", "ui"], ["ui", "clap"]];
  const columns = [["core", "modules", "cables"], ["engine"], ["standalone"], ["ui"], ["clap"]];
  const graph = document.getElementById("graph");
  const readout = document.getElementById("readout");
  let vertical = null, svg, W, H, bw, bh = 44, pos = {}, cables = [], nodes = {}, drag = null;

  const port = (name, out) => {
    const { x, y } = pos[name];
    return vertical ? { x, y: y + (out ? bh / 2 : -bh / 2) } : { x: x + (out ? bw / 2 : -bw / 2), y };
  };
  const drawAll = () => {
    for (const c of cables) {
      const d = c.rope.path();
      c.paths.forEach((p) => p.setAttribute("d", d));
      const a = c.rope.p[0], b = c.rope.p[c.rope.n];
      c.ends[0].setAttribute("cx", a.x); c.ends[0].setAttribute("cy", a.y);
      c.ends[1].setAttribute("cx", b.x); c.ends[1].setAttribute("cy", b.y);
    }
    for (const [name, g] of Object.entries(nodes)) g.setAttribute("transform", `translate(${pos[name].x} ${pos[name].y})`);
  };
  const settleAll = (steps) => { for (const c of cables) c.rope.settle(port(c.a, true), port(c.b, false), steps); };

  function build() {
    vertical = graph.clientWidth < 620;
    W = vertical ? 340 : 900; H = vertical ? 620 : 320; bw = vertical ? 100 : 130;
    pos = {};
    columns.forEach((col, ci) => col.forEach((name, ri) => {
      const main = ((ci + 0.5) / columns.length) * (vertical ? H : W);
      const cross = ((ri + 0.5) / col.length) * (vertical ? W : H);
      pos[name] = vertical ? { x: cross, y: main } : { x: main, y: cross };
    }));
    svg = el("svg", { viewBox: `0 0 ${W} ${H}`, role: "group", "aria-label": "Crate dependency graph. Drag a crate, or focus it and use the arrow keys." });
    const wires = el("g"), nodeG = el("g");
    svg.append(wires, nodeG);
    cables = edges.map(([a, b], i) => {
      const g = el("g", { class: "cable", style: `--c:${CABLE[i % CABLE.length]}` });
      const paths = [el("path", { class: "sheath" }), el("path", { class: "core" }), el("path", { class: "pulse" })];
      const ends = [el("circle", { r: 6, class: "end" }), el("circle", { r: 6, class: "end" })];
      g.append(...paths, ...ends);
      wires.append(g);
      return { a, b, g, paths, ends, rope: new Rope(16, 1.08, 26) };
    });
    nodes = {};
    for (const [name, desc] of Object.entries(crates)) {
      const g = el("g", { class: "crate", tabindex: "0", role: "button", "aria-label": `kabl-${name}: ${desc}` });
      g.append(el("rect", { x: -bw / 2, y: -bh / 2, width: bw, height: bh, rx: 6 }));
      const t = el("text", { y: 5, "text-anchor": "middle", "font-size": vertical ? 14 : 16 });
      t.textContent = name;
      g.append(t);
      const mine = cables.filter((c) => c.a === name || c.b === name);
      const on = () => {
        svg.classList.add("focus"); g.classList.add("on");
        mine.forEach((c) => c.g.classList.add("on"));
        readout.textContent = `kabl-${name} — ${desc}`;
      };
      const off = () => {
        if (drag) return;
        svg.classList.remove("focus"); g.classList.remove("on");
        mine.forEach((c) => c.g.classList.remove("on"));
      };
      g.addEventListener("pointerenter", on);
      g.addEventListener("pointerleave", off);
      g.addEventListener("focus", on);
      g.addEventListener("blur", off);
      g.addEventListener("pointerdown", (e) => {
        const p = svgPoint(svg, e);
        drag = { name, dx: pos[name].x - p.x, dy: pos[name].y - p.y, off };
        g.setPointerCapture(e.pointerId);
        g.classList.add("held");
        e.preventDefault();
      });
      g.addEventListener("keydown", (e) => {
        const k = { ArrowLeft: [-12, 0], ArrowRight: [12, 0], ArrowUp: [0, -12], ArrowDown: [0, 12] }[e.key];
        if (!k) return;
        e.preventDefault();
        move(name, pos[name].x + k[0], pos[name].y + k[1]);
      });
      nodeG.append(g);
      nodes[name] = g;
    }
    svg.addEventListener("pointermove", (e) => {
      if (!drag) return;
      const p = svgPoint(svg, e);
      move(drag.name, p.x + drag.dx, p.y + drag.dy);
    });
    const end = () => {
      if (!drag) return;
      nodes[drag.name].classList.remove("held");
      const { off } = drag;
      drag = null;
      off();
    };
    svg.addEventListener("pointerup", end);
    svg.addEventListener("pointercancel", end);
    settleAll(150);
    drawAll();
    graph.replaceChildren(svg);
  }
  function move(name, x, y) {
    pos[name] = { x: Math.max(bw / 2, Math.min(W - bw / 2, x)), y: Math.max(bh / 2, Math.min(H - bh / 2, y)) };
    if (RM) { settleAll(40); drawAll(); }
    wake();
  }
  document.getElementById("graph-reset").addEventListener("click", () => { vertical = null; build(); wake(); });

  build();
  addScene({
    tick(t) {
      const wind = RM ? 0 : Math.sin(t / 1700) * 0.02;
      for (const c of cables) c.rope.step(port(c.a, true), port(c.b, false), { wind, gravity: 0.4 });
      drawAll();
      if (RM) return false;
      return true;
    },
  }, graph);
  new ResizeObserver(() => { if ((graph.clientWidth < 620) !== vertical) build(); }).observe(graph);
}

// ── Functional cable explainer (a diagram, not kabl's engine).
{
  const root = document.getElementById("fc");
  const svg = root.querySelector(".fc-svg");
  const scope = root.querySelector(".fc-scope");
  const ctx = scope.getContext("2d");
  const play = root.querySelector(".fc-play");
  const input = (n) => root.querySelector(`input[name=${n}]`);
  const out = (n) => root.querySelector(`output[data-for=${n}]`);
  const N = 16;
  const pat = {
    a: Array.from({ length: N }, (_, i) => i % 4 === 0),
    b: Array.from({ length: N }, (_, i) => [0, 2, 3, 6, 8, 10, 11, 14].includes(i)),
  };
  const cells = { a: [], b: [] };
  for (const key of ["a", "b"]) {
    const box = root.querySelector(`.fc-steps[data-pattern=${key}]`);
    for (let i = 0; i < N; i++) {
      const b = document.createElement("button");
      b.type = "button";
      b.className = "step";
      b.setAttribute("aria-label", `${key.toUpperCase()} step ${i + 1}`);
      b.setAttribute("aria-pressed", String(pat[key][i]));
      b.addEventListener("click", () => {
        pat[key][i] = !pat[key][i];
        b.setAttribute("aria-pressed", String(pat[key][i]));
      });
      box.append(b);
      cells[key].push(b);
    }
  }
  const val = { chance: 80, morph: 0, glide: 0 };
  const fmt = { chance: (v) => `${v} %`, morph: (v) => (v / 100).toFixed(2), glide: (v) => `${v} ms` };
  for (const n of Object.keys(val)) {
    input(n).addEventListener("input", () => { val[n] = +input(n).value; out(n).textContent = fmt[n](val[n]); });
  }

  // The cable itself, with physics: a passing pulse twitches it.
  const A = { x: 50, y: 50 }, B = { x: 750, y: 50 };
  const rope = new Rope(30, 1.0, 16);
  rope.settle(A, B);
  const sheath = el("path", { class: "sheath" }), core = el("path", { class: "core" });
  const dotsG = el("g");
  svg.append(sheath, core, el("circle", { class: "plug", cx: A.x, cy: A.y, r: 13 }), el("circle", { class: "plug", cx: B.x, cy: B.y, r: 13 }), dotsG);
  const along = (t) => {
    const f = t * rope.n, i = Math.min(rope.n - 1, Math.floor(f)), r = f - i;
    const p = rope.p[i], q = rope.p[i + 1];
    return { x: p.x + (q.x - p.x) * r, y: p.y + (q.y - p.y) * r, i };
  };

  let playing = false, step = -1, acc = 0, level = 0, target = 0;
  const STEP_MS = 125; // a 16th note at 120 BPM
  const dots = [];
  const trace = new Float32Array(240);
  let ti = 0;
  function pulse() {
    step = (step + 1) % N;
    for (const k of ["a", "b"]) cells[k].forEach((c, i) => c.classList.toggle("now", i === step));
    const m = val.morph / 100;
    const lv = (1 - m) * +pat.a[step] + m * +pat.b[step];
    const passed = Math.random() * 100 < val.chance;
    target = passed ? lv : 0;
    if (lv > 0) {
      const d = el("circle", { class: passed ? "dot" : "dot dropped", r: 5 + 6 * lv });
      d.style.opacity = String(0.35 + 0.65 * lv);
      dotsG.append(d);
      dots.push({ el: d, t: 0, passed, lv });
    }
  }
  function drawScope() {
    const w = scope.width, h = scope.height;
    ctx.clearRect(0, 0, w, h);
    ctx.strokeStyle = "rgba(255,255,255,.08)";
    ctx.lineWidth = 1;
    for (let y = 0; y <= 4; y++) { ctx.beginPath(); ctx.moveTo(0, (h - 8) * (y / 4) + 4); ctx.lineTo(w, (h - 8) * (y / 4) + 4); ctx.stroke(); }
    ctx.strokeStyle = "#9fe39f";
    ctx.lineWidth = 2 * (window.devicePixelRatio || 1);
    ctx.beginPath();
    for (let k = 0; k < trace.length; k++) {
      const v = trace[(ti + k) % trace.length];
      const x = (k / (trace.length - 1)) * w, y = h - 4 - v * (h - 8);
      k ? ctx.lineTo(x, y) : ctx.moveTo(x, y);
    }
    ctx.stroke();
  }
  const sizeScope = () => {
    const dpr = window.devicePixelRatio || 1;
    scope.width = Math.max(1, scope.clientWidth * dpr);
    scope.height = Math.max(1, scope.clientHeight * dpr);
    drawScope();
  };
  new ResizeObserver(sizeScope).observe(scope);

  const scene = {
    tick(t, dt) {
      if (playing) {
        acc += dt;
        while (acc >= STEP_MS) { acc -= STEP_MS; pulse(); }
        // Glide: slew the level toward the target.
        if (val.glide === 0) level = target;
        else {
          const max = dt / val.glide;
          level += Math.max(-max, Math.min(max, target - level));
        }
        trace[ti] = level;
        ti = (ti + 1) % trace.length;
      }
      for (let k = dots.length - 1; k >= 0; k--) {
        const d = dots[k];
        d.t += dt / (d.passed ? 700 : 260);
        const p = along(Math.min(1, d.passed ? d.t : d.t * 0.22));
        d.el.setAttribute("cx", p.x);
        d.el.setAttribute("cy", p.y);
        if (d.passed && !RM) { const q = rope.p[Math.max(1, Math.min(rope.n - 1, p.i))]; q.y += 0.9 * d.lv; }
        if (!d.passed) d.el.style.opacity = String(Math.max(0, 1 - d.t));
        if (d.t >= 1) { d.el.remove(); dots.splice(k, 1); }
      }
      rope.step(A, B, { gravity: 0.35, damping: 0.95 });
      const path = rope.path();
      sheath.setAttribute("d", path);
      core.setAttribute("d", path);
      drawScope();
      return playing || dots.length > 0 || rope.energy() > 0.05;
    },
  };
  addScene(scene, root);
  scene.tick(0, 0);
  const setPlaying = (on) => {
    playing = on;
    play.setAttribute("aria-pressed", String(on));
    play.textContent = on ? "Pause" : "Play";
    if (!on) for (const k of ["a", "b"]) cells[k].forEach((c) => c.classList.remove("now"));
    wake();
  };
  play.addEventListener("click", () => setPlaying(!playing));
  // Starts by itself the first time it scrolls into view, unless motion is reduced.
  if (!RM) {
    const io = new IntersectionObserver(([e]) => { if (e.isIntersecting) { setPlaying(true); io.disconnect(); } }, { threshold: 0.4 });
    io.observe(root);
  }
}

// ── Panels slide in as they scroll into view.
if (!RM && "IntersectionObserver" in window) {
  document.documentElement.classList.add("reveal-on");
  const io = new IntersectionObserver((es) => es.forEach((e) => { if (e.isIntersecting) { e.target.classList.add("in"); io.unobserve(e.target); } }), { threshold: 0.08 });
  document.querySelectorAll("main .panel").forEach((p) => io.observe(p));
}

// ── Module LEDs flicker on, one after another, when the rack comes into view.
{
  const rack = document.getElementById("rack");
  const io = new IntersectionObserver(([e]) => {
    if (!e.isIntersecting) return;
    io.disconnect();
    rack.querySelectorAll(".led").forEach((l, i) => setTimeout(() => l.classList.add("lit"), RM ? 0 : 60 * i));
  }, { threshold: 0.3 });
  io.observe(rack);
}

// ── Screenshots open full size.
{
  const box = document.getElementById("lightbox");
  const img = box.querySelector("img");
  document.querySelectorAll(".shot img").forEach((im) => {
    const b = document.createElement("button");
    b.className = "zoom";
    b.type = "button";
    b.setAttribute("aria-label", `View full size: ${im.alt}`);
    im.replaceWith(b);
    b.append(im);
    b.addEventListener("click", () => { img.src = im.src; img.alt = im.alt; box.showModal(); });
  });
  box.addEventListener("click", (e) => { if (e.target === box) box.close(); });
}
