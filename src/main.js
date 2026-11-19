import "./style.css";

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
const rack = document.getElementById("rack");
let n = 0;
for (const [family, kinds] of families) {
  const g = document.createElement("div");
  g.className = "family";
  g.innerHTML = `<h3 class="mono">${family}</h3><ul>${kinds
    .map((k) => `<li><span class="idx mono">${String(++n).padStart(2, "0")}</span>${k}</li>`)
    .join("")}</ul>`;
  rack.append(g);
}

// ── Crate graph: workspace crates and their direct dependencies (from each
// crate's Cargo.toml, transitive edges removed). Descriptions from the README.
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
const colors = ["#e8b628", "#d9534a", "#4a90d9", "#5cb85c", "#c27bd6", "#e8893a"];

const graph = document.getElementById("graph");
const readout = document.getElementById("readout");
const NS = "http://www.w3.org/2000/svg";
const el = (tag, attrs = {}) => {
  const e = document.createElementNS(NS, tag);
  for (const [k, v] of Object.entries(attrs)) e.setAttribute(k, v);
  return e;
};

function draw() {
  const vertical = graph.clientWidth < 620;
  const W = vertical ? 340 : 900, H = vertical ? 620 : 300, bw = vertical ? 100 : 130, bh = 44;
  const pos = {};
  columns.forEach((col, ci) => {
    col.forEach((name, ri) => {
      const main = ((ci + 0.5) / columns.length) * (vertical ? H : W);
      const cross = ((ri + 0.5) / col.length) * (vertical ? W : H);
      pos[name] = vertical ? { x: cross, y: main } : { x: main, y: cross };
    });
  });
  const svg = el("svg", { viewBox: `0 0 ${W} ${H}`, role: "group", "aria-label": "Crate dependency graph" });
  const wires = el("g"), nodes = el("g");
  svg.append(wires, nodes);

  // Cables sag under "gravity", like patch cables.
  const cableOf = {};
  edges.forEach(([a, b], i) => {
    const A = pos[a], B = pos[b];
    const [x1, y1, x2, y2] = vertical ? [A.x, A.y + bh / 2, B.x, B.y - bh / 2] : [A.x + bw / 2, A.y, B.x - bw / 2, B.y];
    const sag = 28 + Math.hypot(x2 - x1, y2 - y1) * 0.12;
    const d = vertical
      ? `M${x1} ${y1} C ${x1 + sag} ${y1 + 30}, ${x2 + sag} ${y2 - 30}, ${x2} ${y2}`
      : `M${x1} ${y1} C ${x1 + 30} ${y1 + sag}, ${x2 - 30} ${y2 + sag}, ${x2} ${y2}`;
    const g = el("g", { class: "cable", style: `--c:${colors[i % colors.length]}` });
    g.append(el("path", { d, class: "sheath" }), el("path", { d, class: "core" }), el("path", { d, class: "pulse" }));
    g.append(el("circle", { cx: x1, cy: y1, r: 6, class: "end" }), el("circle", { cx: x2, cy: y2, r: 6, class: "end" }));
    wires.append(g);
    (cableOf[a] ??= []).push(g);
    (cableOf[b] ??= []).push(g);
  });

  for (const [name, desc] of Object.entries(crates)) {
    const { x, y } = pos[name];
    const g = el("g", { class: "crate", tabindex: "0", role: "button", "aria-label": `kabl-${name}: ${desc}` });
    g.append(el("rect", { x: x - bw / 2, y: y - bh / 2, width: bw, height: bh, rx: 6 }));
    const t = el("text", { x, y: y + 5, "text-anchor": "middle", "font-size": vertical ? 14 : 16 });
    t.textContent = name;
    g.append(t);
    const on = () => {
      svg.classList.add("focus");
      g.classList.add("on");
      (cableOf[name] ?? []).forEach((c) => c.classList.add("on"));
      readout.textContent = `kabl-${name} — ${desc}`;
    };
    const off = () => {
      svg.classList.remove("focus");
      g.classList.remove("on");
      (cableOf[name] ?? []).forEach((c) => c.classList.remove("on"));
    };
    g.addEventListener("pointerenter", on);
    g.addEventListener("pointerleave", off);
    g.addEventListener("focus", on);
    g.addEventListener("blur", off);
    nodes.append(g);
  }
  graph.replaceChildren(svg);
}
draw();
let last = graph.clientWidth < 620;
new ResizeObserver(() => {
  const now = graph.clientWidth < 620;
  if (now !== last) { last = now; draw(); }
}).observe(graph);
