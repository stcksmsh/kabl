// A patch cable as a Verlet rope: n segments between two pinned ends.
// Length follows the distance between the ends (plus slack), so the cable always
// sags naturally however far apart its plugs are.
export class Rope {
  constructor(n = 18, slack = 1.18, extra = 30) {
    this.n = n;
    this.slack = slack;
    this.extra = extra;
    this.p = [];
    this.o = [];
  }

  reset(a, b) {
    this.p = [];
    this.o = [];
    for (let i = 0; i <= this.n; i++) {
      const t = i / this.n;
      const pt = { x: a.x + (b.x - a.x) * t, y: a.y + (b.y - a.y) * t };
      this.p.push(pt);
      this.o.push({ ...pt });
    }
  }

  step(a, b, { gravity = 0.5, damping = 0.97, iterations = 14, wind = 0 } = {}) {
    if (!this.p.length) this.reset(a, b);
    const { p, o, n } = this;
    const seg = (Math.hypot(b.x - a.x, b.y - a.y) * this.slack + this.extra) / n;
    for (let i = 1; i < n; i++) {
      const vx = (p[i].x - o[i].x) * damping;
      const vy = (p[i].y - o[i].y) * damping;
      o[i].x = p[i].x;
      o[i].y = p[i].y;
      p[i].x += vx + wind;
      p[i].y += vy + gravity;
    }
    for (let k = 0; k < iterations; k++) {
      p[0].x = a.x; p[0].y = a.y;
      p[n].x = b.x; p[n].y = b.y;
      for (let i = 0; i < n; i++) {
        const q = p[i], r = p[i + 1];
        const dx = r.x - q.x, dy = r.y - q.y;
        const d = Math.hypot(dx, dy) || 1e-6;
        const diff = (d - seg) / d;
        const qPinned = i === 0, rPinned = i + 1 === n;
        const wq = qPinned ? 0 : rPinned ? 1 : 0.5;
        const wr = rPinned ? 0 : qPinned ? 1 : 0.5;
        q.x += dx * diff * wq; q.y += dy * diff * wq;
        r.x -= dx * diff * wr; r.y -= dy * diff * wr;
      }
    }
    p[0].x = a.x; p[0].y = a.y;
    p[n].x = b.x; p[n].y = b.y;
  }

  // Settle without animation (reduced motion / first paint).
  settle(a, b, steps = 120) {
    for (let i = 0; i < steps; i++) this.step(a, b, { damping: 0.8 });
  }

  // Smooth path through the points (quadratic curves via midpoints).
  path() {
    const p = this.p;
    let d = `M${p[0].x.toFixed(1)} ${p[0].y.toFixed(1)}`;
    for (let i = 1; i < p.length - 1; i++) {
      const mx = (p[i].x + p[i + 1].x) / 2, my = (p[i].y + p[i + 1].y) / 2;
      d += ` Q${p[i].x.toFixed(1)} ${p[i].y.toFixed(1)} ${mx.toFixed(1)} ${my.toFixed(1)}`;
    }
    const l = p[p.length - 1];
    return d + ` L${l.x.toFixed(1)} ${l.y.toFixed(1)}`;
  }

  // Kinetic energy, to let a scene sleep once the cable has come to rest.
  energy() {
    let e = 0;
    for (let i = 1; i < this.n; i++) e += Math.abs(this.p[i].x - this.o[i].x) + Math.abs(this.p[i].y - this.o[i].y);
    return e;
  }
}

export const reducedMotion = () => matchMedia("(prefers-reduced-motion: reduce)").matches;

// Pointer position in an SVG's own coordinates.
export function svgPoint(svg, e) {
  const pt = svg.createSVGPoint();
  pt.x = e.clientX;
  pt.y = e.clientY;
  return pt.matrixTransform(svg.getScreenCTM().inverse());
}
