// printCAD's site: the pinned 3D story, the two Check viewers, scroll
// reveals, the section ruler, the console that types itself, the store's
// packages and the downloads.
import { Stage, loadModels, mat4, smooth, ease, gridLines, polyline } from "./gl.js";

const still = matchMedia("(prefers-reduced-motion: reduce)").matches;
const root = document.documentElement;
const clamp = (v, a = 0, b = 1) => Math.min(Math.max(v, a), b);
const lerp = (a, b, t) => a + (b - a) * t;

addEventListener("pointermove", (e) => {
  root.style.setProperty("--mx", e.clientX + "px");
  root.style.setProperty("--my", e.clientY + "px");
});

// ---- Reveals, header and ruler -------------------------------------------

const seen = new IntersectionObserver(
  (entries) => {
    for (const entry of entries) {
      if (!entry.isIntersecting) continue;
      entry.target.classList.add("in");
      seen.unobserve(entry.target);
    }
  },
  { threshold: 0.12, rootMargin: "0px 0px -6% 0px" },
);
function watch(el) {
  const siblings = [...el.parentElement.children].filter((c) => c.classList.contains("reveal"));
  el.style.setProperty("--delay", `${Math.min(siblings.indexOf(el), 6) * 0.08}s`);
  seen.observe(el);
}
document.querySelectorAll(".reveal").forEach(watch);

const header = document.querySelector(".top");
const fill = document.querySelector(".ruler-fill");
const marks = [...document.querySelectorAll(".ruler a")];
const sections = marks.map((a) => document.querySelector(a.getAttribute("href")));
const story = document.querySelector(".story");
const panels = [...document.querySelectorAll(".panel")];
const chips = [...document.querySelectorAll(".stage-steps span")];
let storyT = 0;

function onScroll() {
  header.classList.toggle("scrolled", scrollY > 30);
  // The section across the middle of the window is the current one; the
  // fill runs to its dot and on toward the next as that one comes up.
  const mid = innerHeight * 0.5;
  const tops = sections.map((s) => (s ? s.getBoundingClientRect().top + sectionLead(s) : Infinity));
  let current = 0;
  tops.forEach((t, i) => {
    if (t < mid) current = i;
  });
  const last = root.scrollHeight - innerHeight - scrollY < 4;
  if (last) current = sections.length - 1;
  const next = tops[current + 1];
  const part = next === undefined || last ? 0 : clamp((mid - tops[current]) / (next - tops[current]));
  fill.style.height = `${((current + part) / (marks.length - 1)) * 100}%`;
  marks.forEach((m, i) => m.classList.toggle("on", i === current));

  const box = story.getBoundingClientRect();
  storyT = clamp(-box.top / (box.height - innerHeight), 0, 0.9999);
  const at = Math.floor(storyT * panels.length);
  panels.forEach((p, i) => {
    p.classList.toggle("on", i === at);
    p.classList.toggle("past", i < at);
  });
  chips.forEach((c) => {
    const n = Number(c.dataset.step);
    c.classList.toggle("on", n === at);
    c.classList.toggle("done", n < at);
  });
}
// Where a section's own heading starts, below its padding.
function sectionLead(s) {
  const first = s.querySelector(".eyebrow");
  return first ? first.getBoundingClientRect().top - s.getBoundingClientRect().top : 0;
}
// The app's picture under the downloads drifts up a little as it comes in.
const shot = document.querySelector(".app-shot");
addEventListener("scroll", () => {
  const r = shot.getBoundingClientRect();
  const p = clamp(1 - r.top / innerHeight, 0, 1.5);
  shot.style.setProperty("--drift", `${(1 - p) * 40}px`);
}, { passive: true });
addEventListener("scroll", onScroll, { passive: true });
addEventListener("resize", onScroll);
// On a narrow screen the story's words sit under the model: their box is
// as tall as the longest step needs, and the model takes the rest.
const storyText = document.querySelector(".story-text");
function fitStoryText() {
  if (innerWidth > 1000) return storyText.style.removeProperty("--text-h");
  const heights = panels.map((p) => {
    p.style.bottom = "auto";
    const h = p.offsetHeight;
    p.style.bottom = "";
    return h;
  });
  storyText.style.setProperty("--text-h", `${Math.max(...heights) + 8}px`);
}
addEventListener("resize", fitStoryText);
document.fonts.ready.then(fitStoryText);
fitStoryText();
onScroll();

for (const el of document.querySelectorAll(".spot")) {
  el.addEventListener("pointermove", (e) => {
    const r = el.getBoundingClientRect();
    el.style.setProperty("--x", `${e.clientX - r.left}px`);
    el.style.setProperty("--y", `${e.clientY - r.top}px`);
  });
}

// ---- Labels over a stage ----------------------------------------------------

function labels(stage, container) {
  const list = [];
  return {
    add(text, at, kind = "") {
      const el = document.createElement("span");
      el.className = `label ${kind}`;
      el.textContent = text;
      container.append(el);
      const l = { el, at, show: 0 };
      list.push(l);
      return l;
    },
    update() {
      for (const l of list) {
        const [x, y, z] = stage.project(l.at);
        // Kept inside the stage, so a label near its edge is never cut.
        const half = l.el.offsetWidth / 2 + 6, halfH = l.el.offsetHeight / 2 + 6;
        l.el.style.left = `${clamp(x, half, stage.cssW - half)}px`;
        l.el.style.top = `${clamp(y, halfH, stage.cssH - halfH)}px`;
        l.el.style.opacity = z < 1 ? l.show : 0;
      }
    },
  };
}

// ---- The console types a script, then runs it --------------------------------

const script = [
  ["c", "-- a plate with a hole, from the console\n"],
  ["k", "local "], ["", "s = pc.sketch.new{plane = "], ["s", '"XY"'], ["", "}\n"],
  ["", "pc.sketch.rect{sketch = s, width = "], ["n", "60"], ["", ", height = "], ["n", "40"], ["", "}\n"],
  ["", "pc.sketch.circle{sketch = s, x = "], ["n", "30"], ["", ", y = "], ["n", "20"], ["", ", radius = "], ["n", "6"], ["", "}\n"],
  ["k", "local "], ["", "pad = pc.design.pad{sketch = s, length = "], ["n", "8"], ["", "}\n"],
  ["k", "return "], ["", "pc.doc.rebuild()\n"],
];
const answer = [
  ["c", "\n> run\n"],
  ["ok", "✓ "], ["", "sketch   fully constrained\n"],
  ["ok", "✓ "], ["", "pad      60 × 40 × 8 mm\n"],
  ["ok", "✓ "], ["", "rebuilt  nothing failed, one undo step\n"],
];
{
  const term = document.querySelector("#term");
  const code = term.querySelector("code");
  const state = term.querySelector(".term-state");
  const type = async (parts, speed) => {
    const caret = document.createElement("span");
    caret.className = "caret";
    for (const [kind, text] of parts) {
      const span = document.createElement("span");
      if (kind) span.className = kind;
      code.append(span);
      span.after(caret);
      for (const ch of text) {
        span.textContent += ch;
        if (!still) await new Promise((r) => setTimeout(r, speed + Math.random() * speed));
      }
    }
    caret.remove();
  };
  new IntersectionObserver(async (entries, obs) => {
    if (!entries.some((e) => e.isIntersecting)) return;
    obs.disconnect();
    state.textContent = "typing";
    await type(script, 12);
    state.textContent = "running";
    await new Promise((r) => setTimeout(r, still ? 0 : 450));
    await type(answer, 5);
    state.textContent = "done";
  }, { threshold: 0.4 }).observe(term);
}

// ---- The store's packages ------------------------------------------------------

// A package's badge: the icon of its first category the page knows.
const CATEGORY_ICONS = {
  fasteners: "hole", parts: "workbench-part-design", printing: "print-bed", "import-export": "file-document",
  utilities: "script", assembly: "workbench-assembly", surfaces: "workbench-surface", sketch: "workbench-sketcher",
  mesh: "workbench-mesh", generators: "involute-gear", analysis: "measure",
};
const categoryIcon = (cats = []) => CATEGORY_ICONS[cats.find((c) => CATEGORY_ICONS[c])] ?? "workbench-part-design";

{
  const store = document.querySelector("#store");
  store.innerHTML = '<div class="pkg skeleton"></div>'.repeat(3);
  fetch("https://gilbertorconde.github.io/PrintCAD-wb-repo/index.json")
    .then((r) => (r.ok ? r.json() : Promise.reject()))
    .then((index) => {
      store.innerHTML = "";
      for (const p of index.packages ?? []) {
        const card = document.createElement("a");
        card.className = "pkg reveal";
        card.href = `https://github.com/${p.repository}`;
        const head = document.createElement("div");
        head.className = "head";
        const badge = document.createElement("i");
        badge.className = "badge";
        badge.style.setProperty("--i", `url(assets/icons/${categoryIcon(p.categories)}.svg)`);
        const title = document.createElement("div");
        const h3 = document.createElement("h3");
        h3.textContent = p.name;
        const ver = document.createElement("span");
        ver.className = "ver";
        ver.textContent = p.release ? `v${p.release.version}` : "";
        title.append(h3, ver);
        head.append(badge, title);
        const desc = document.createElement("p");
        desc.textContent = p.description ?? "";
        const tagsEl = document.createElement("div");
        tagsEl.className = "tags";
        for (const c of p.categories ?? []) {
          const s = document.createElement("span");
          s.textContent = c;
          tagsEl.append(s);
        }
        const by = document.createElement("span");
        by.className = "by";
        by.textContent = `${p.repository}`;
        card.append(head, desc, tagsEl, by);
        store.append(card);
        watch(card);
      }
    })
    .catch(() => {
      store.innerHTML = '<p class="note">The store could not be reached; browse it from inside printCAD.</p>';
    });
}

// ---- Downloads -------------------------------------------------------------------

{
  const os = /Win/.test(navigator.platform) ? "windows" : /Mac/.test(navigator.platform) ? "macos" : "linux";
  document.querySelector(`.dl[data-platform="${os}"]`)?.classList.add("mine");
  fetch("https://api.github.com/repos/gilbertorconde/printCAD/releases/latest")
    .then((r) => (r.ok ? r.json() : Promise.reject()))
    .then((release) => {
      const ends = { linux: "linux-x86_64.tar.gz", windows: "windows-x86_64.zip", macos: "macos-universal.dmg" };
      for (const a of document.querySelectorAll(".dl")) {
        const asset = release.assets.find((x) => x.name.endsWith(ends[a.dataset.platform]));
        if (asset) a.href = asset.browser_download_url;
      }
      document.querySelector("#release-line").textContent =
        `Version ${release.tag_name.replace(/^v/, "")} · built in Rust on Vulkan, with a pure-Rust geometry kernel`;
    })
    .catch(() => {});
}

// ---- The story ------------------------------------------------------------

// The 3D below waits for the models; everything above runs without them.
const models = await loadModels();

{
  const canvas = document.querySelector("#story-stage");
  const stage = new Stage(canvas, { target: [20, 15, 12], distance: 150, yaw: -1.95, pitch: 0.42, fov: 0.42, fit: 1.05 });
  if (stage.gl) {
    stage.spin = 0;
    const tags = labels(stage, document.querySelector("#story-labels"));

    // The bracket's side, drawn on its XZ plane: the sketch it is padded from.
    const t = 5, foot = 40, height = 30, w = 30;
    const profile = [[0, 0, 0], [foot, 0, 0], [foot, 0, t], [t, 0, t], [t, 0, height], [0, 0, height]];
    const sketch = polyline(profile);
    const sketchLines = stage.addLines(sketch.points, { t: sketch.t, color: [0.44, 0.83, 0.79, 1], progress: 0 });
    // Dimension lines, dashed by drawing them in short pieces.
    const dash = (a, b, n = 14) => {
      const pts = [];
      for (let i = 0; i < n; i++) {
        const s = i / n, e = (i + 0.55) / n;
        pts.push(...a.map((v, k) => lerp(v, b[k], s)), ...a.map((v, k) => lerp(v, b[k], e)));
      }
      return pts;
    };
    const dims = stage.addLines(
      [...dash([0, 0, -6], [foot, 0, -6]), ...dash([-6, 0, 0], [-6, 0, height]), ...dash([foot + 6, 0, 0], [foot + 6, 0, t], 4), ...dash([0, 0, height + 6], [t, 0, height + 6], 4)],
      { color: [0.9, 0.64, 0.31, 0] },
    );
    const grid = stage.addLines(gridLines(50, -25, 70, 10), { color: [0.26, 0.33, 0.42, 0], fade: 95, fadeAt: [50, -25, 0] });
    const bedEdge = stage.addLines([
      -20, -95, 0, 120, -95, 0, 120, -95, 0, 120, 45, 0,
      120, 45, 0, -20, 45, 0, -20, 45, 0, -20, -95, 0,
    ], { color: [0.31, 0.64, 0.9, 0] });

    const pad = stage.addMesh(models.pad, { alpha: 0, color: [0.31, 0.64, 0.9], edges: 0.9, edgeColor: [0.44, 0.83, 0.79], grow: [1, 0, 1] });
    const part = stage.addMesh(models.bracket, { alpha: 0, show: false });
    const others = [
      { mesh: models.gear, at: [82, 20, 0] },
      { mesh: models.sprocket, at: [16, -48, 0] },
      { mesh: models.nut, at: [55, -62, 0] },
    ].map((o, i) => ({ ...o, i, item: stage.addMesh(o.mesh, { alpha: 0, show: false }) }));
    // Each part's shadow on the bed, a little wider than its footprint.
    const shadowOf = (m, at) => {
      const s = stage.addShadow((m.hi[0] - m.lo[0]) * 0.62, (m.hi[1] - m.lo[1]) * 0.62);
      s.model = mat4.translate(at[0] + (m.lo[0] + m.hi[0]) / 2, at[1] + (m.lo[1] + m.hi[1]) / 2, 0);
      return s;
    };
    const partShadow = shadowOf(models.bracket, [0, 0, 0]);
    for (const o of others) o.shadow = shadowOf(o.mesh, o.at);

    const L = {
      d40: tags.add("40", [foot / 2, 0, -6], "dim"),
      d30: tags.add("30", [-6, 0, height / 2], "dim"),
      d5a: tags.add("5", [foot + 6, 0, t / 2], "dim"),
      d5b: tags.add("5", [t / 2, 0, height + 6], "dim"),
      ok: tags.add("fully constrained", [foot * 0.62, 0, height * 0.72], "ok"),
      perp: tags.add("perpendicular", [t + 9, 0, t + 9], "ok"),
      horiz: tags.add("horizontal", [foot * 0.74, 0, t + 4], "ok"),
      dots: profile.map((p) => tags.add("", p, "dot")),
      pad: tags.add("Pad · 30 mm", [foot, w / 2, t + 2], "feat"),
      hole: tags.add("3 × Hole ⌀5", [27, 15, t + 3], "feat"),
      fillet: tags.add("Fillet R4", [t + 4, 2, t + 4], "feat"),
      bed: tags.add("4 parts · one plate", [50, -95, 0], "ok"),
    };

    const begun = performance.now();
    const cam = { target: [20, 15, 12], distance: 150, pitch: 0.42, yaw: -1.95 };
    stage.cam = { ...stage.cam, ...cam };
    let userYaw = 0;
    let lastYaw = stage.cam.yaw;

    stage.onFrame = () => {
      const T = storyT * panels.length; // 0..5
      const since = still ? 9 : (performance.now() - begun) / 1000;
      // The opening draws the sketch on its own; scrolling takes over.
      const draw = Math.max(smooth(0.3, 2.0, since), clamp(T));
      sketchLines.progress = draw;
      const dimIn = Math.max(smooth(1.9, 2.6, since), clamp(T)) * (1 - smooth(2.15, 2.5, T));
      dims.color[3] = dimIn * 0.9;
      [L.d40, L.d30, L.d5a, L.d5b].forEach((l) => (l.show = dimIn));
      L.dots.forEach((l, i) => (l.show = clamp(draw * profile.length - i) * (1 - smooth(2.0, 2.3, T))));
      const constrained = smooth(1, 1.3, T) * (1 - smooth(1.95, 2.2, T));
      L.ok.show = constrained;
      L.perp.show = constrained;
      L.horiz.show = constrained;

      // The pad grows out of the sketch, see-through, then solid.
      const grow = smooth(2.05, 2.7, T);
      pad.grow = [1, Math.max(grow, 0.0001), 1];
      pad.alpha = T < 2 ? 0 : lerp(0.35, 1, smooth(2.6, 2.95, T));
      pad.color = [lerp(0.31, 0.62, smooth(2.6, 2.95, T)), lerp(0.64, 0.66, smooth(2.6, 2.95, T)), lerp(0.9, 0.72, smooth(2.6, 2.95, T))];
      pad.edgeColor = smooth(2.6, 2.95, T) > 0.5 ? [0.06, 0.07, 0.09] : [0.44, 0.83, 0.79];
      pad.show = T < 3.05;
      sketchLines.color[3] = 1 - smooth(2.3, 2.8, T) * 0.85;
      L.pad.show = smooth(2.3, 2.5, T) * (1 - smooth(2.85, 3.0, T));

      // The holes and the fillet arrive on the same solid.
      part.show = T >= 3;
      part.alpha = part.show ? 1 : 0;
      part.glow2 = smooth(3.0, 3.2, T) * (1 - smooth(3.45, 3.75, T)) * 0.85;
      part.glow1 = smooth(3.3, 3.5, T) * (1 - smooth(3.75, 4.0, T)) * 0.85;
      L.hole.show = smooth(3.05, 3.2, T) * (1 - smooth(3.5, 3.65, T));
      L.fillet.show = smooth(3.35, 3.5, T) * (1 - smooth(3.8, 3.95, T));

      // On the bed: the grid comes up and the other parts drop in.
      const bed = smooth(4.0, 4.4, T);
      grid.color[3] = bed * 0.9;
      bedEdge.color[3] = bed * 0.8;
      for (const o of others) {
        const s = smooth(4.1 + o.i * 0.12, 4.45 + o.i * 0.12, T);
        o.item.show = s > 0;
        o.item.alpha = s > 0 ? Math.min(1, s * 1.6) : 0;
        o.item.model = mat4.translate(o.at[0], o.at[1], o.at[2] + (1 - ease(s)) * 50);
        o.shadow.alpha = ease(s) * 0.85;
      }
      L.bed.show = smooth(4.6, 4.8, T);
      partShadow.alpha = bed * 0.85;

      // The camera eases from the sketch's plane to the part, then the bed.
      const want = {
        target: T < 3.9 ? [20, 15, 12] : [lerp(20, 50, bed), lerp(15, -25, bed), lerp(12, -6, bed)],
        distance: lerp(T < 2 ? 135 : 150, 480, bed),
        pitch: lerp(lerp(0.2, 0.5, smooth(1.8, 2.8, T)), 0.95, bed),
        yaw: lerp(lerp(-1.7, -2.35, smooth(1.8, 3.4, T)), -2.15, bed),
      };
      if (stage.drag) userYaw += stage.cam.yaw - lastYaw;
      else userYaw *= 0.96;
      const k = 0.08;
      stage.cam.target = stage.cam.target.map((v, i) => lerp(v, want.target[i], k));
      stage.cam.distance = lerp(stage.cam.distance, want.distance, k);
      if (!stage.drag) {
        stage.cam.pitch = lerp(stage.cam.pitch, want.pitch, k);
        stage.cam.yaw = lerp(stage.cam.yaw, want.yaw + userYaw + Math.sin(since * 0.25) * 0.08, k);
      }
      lastYaw = stage.cam.yaw;
    };
    stage.afterFrame = () => tags.update();
  }
}

// ---- Surfaces: a bottle, shaded, by curvature, or in zebra stripes -----------

{
  const stage = new Stage(document.querySelector("#surface-stage"), { target: [0, 0, 39], distance: 188, yaw: -0.9, pitch: 0.26, fov: 0.5, fit: 1.0 });
  if (stage.gl) {
    stage.spin = still ? 0 : 0.15;
    const vase = stage.addMesh(models.vase, { mode: 2, edges: 0.3, stripes: 12 });
    const legend = document.querySelector("#curv-legend");
    const buttons = [...document.querySelectorAll("#surface-modes button")];
    for (const b of buttons) {
      b.addEventListener("click", () => {
        vase.mode = Number(b.dataset.mode);
        buttons.forEach((x) => x.classList.toggle("on", x === b));
        legend.classList.toggle("off", vase.mode !== 2);
      });
    }
  }
}

// ---- Check: a section to drag through a plate ------------------------------

{
  const stage = new Stage(document.querySelector("#section-stage"), { target: [50, 15, 4], distance: 118, yaw: -1.3, pitch: 0.55, fov: 0.5, fit: 1.9 });
  if (stage.gl) {
    stage.spin = 0;
    const plate = stage.addMesh(models.plate, { color: [0.6, 0.64, 0.7], edges: 0.4 });
    const ghost = stage.addMesh(models.plate, { color: [0.31, 0.64, 0.9], alpha: 0.07, edges: 0 });
    const cut = document.querySelector("#section-cut");
    stage.onFrame = (time) => {
      const y = Number(cut.value) * 30;
      plate.clip = [0, -1, 0, y];
      ghost.clip = [0, 1, 0, -y];
      if (!stage.drag) stage.cam.yaw = -1.3 + Math.sin(time * 0.3) * 0.22;
    };
  }
}

// ---- Print: a grip with a texture pressed in, and a nut trap ------------------

// The app's patterns (surface_texture), height 0..1 over a tile.
const fr = (x) => x - Math.floor(x);
const tri = (x) => 1 - Math.abs(2 * fr(x) - 1);
const sm = (x) => x * x * (3 - 2 * x);
const groove = (t) => sm(clamp(1 - t / 0.18));
const PATTERNS = {
  knurl: (u, v) => Math.min(tri(u + v), tri(u - v)),
  ribs: (u) => sm(tri(u)),
  dots: (u, v) => {
    const r = Math.max(1 - Math.hypot(fr(u) - 0.5, fr(v) - 0.5) / 0.38, 0);
    return Math.sqrt(r * (2 - r));
  },
  hex: (u, v) => {
    const s3 = Math.sqrt(3);
    const x = fr(u), y = fr(v) * s3;
    let first = Infinity, second = Infinity;
    for (const [cx, cy] of [[0, 0], [1, 0], [0.5, s3 / 2], [0, s3], [1, s3]]) {
      const d = Math.hypot(x - cx, y - cy);
      if (d < first) [second, first] = [first, d];
      else if (d < second) second = d;
    }
    return 1 - groove(clamp((second - first) / 0.5));
  },
  waves: (u) => 0.5 + 0.5 * Math.sin(2 * Math.PI * u),
};

// A cylinder 20 mm across and 30 tall, its side pressed by `pattern` to
// `depth` mm, the ends kept flat as the app keeps a rim.
function grip(pattern, depth) {
  const R = 10, H = 30, AROUND = 360, UP = 150, TILES = 20, FLAT = 2;
  const tile = (2 * Math.PI * R) / TILES;
  const side = (AROUND + 1) * (UP + 1);
  const pos = new Float32Array((side + 2 * (AROUND + 2)) * 3);
  const nrm = new Float32Array(pos.length);
  const h = PATTERNS[pattern];
  const at = (i, j) => {
    const a = (i / AROUND) * 2 * Math.PI, z = (j / UP) * H;
    const keep = smoothRim(z, H, FLAT);
    const r = R + depth * keep * h((i / AROUND) * TILES, z / tile);
    return [r * Math.cos(a), r * Math.sin(a), z];
  };
  for (let j = 0; j <= UP; j++)
    for (let i = 0; i <= AROUND; i++) {
      const k = (j * (AROUND + 1) + i) * 3;
      const p = at(i, j);
      const du = sub3(at(i + 1, j), at(i - 1, j));
      const dv = sub3(at(i, Math.min(j + 1, UP)), at(i, Math.max(j - 1, 0)));
      const n = norm3(cross3(du, dv));
      pos.set(p, k);
      nrm.set(n, k);
    }
  // The two ends: a fan each.
  let k = side * 3;
  for (const [z, nz] of [[0, -1], [H, 1]]) {
    pos.set([0, 0, z], k);
    nrm.set([0, 0, nz], k);
    k += 3;
    for (let i = 0; i <= AROUND; i++) {
      const a = (i / AROUND) * 2 * Math.PI;
      pos.set([R * Math.cos(a), R * Math.sin(a), z], k);
      nrm.set([0, 0, nz], k);
      k += 3;
    }
  }
  return { pos, nrm };
}
const smoothRim = (z, H, flat) => sm(clamp(z / flat)) * sm(clamp((H - z) / flat));
const sub3 = (a, b) => [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
const cross3 = (a, b) => [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]];
const norm3 = (a) => {
  const n = Math.hypot(a[0], a[1], a[2]) || 1;
  return [a[0] / n, a[1] / n, a[2] / n];
};
function gripIndices() {
  const AROUND = 360, UP = 150;
  const side = (AROUND + 1) * (UP + 1);
  const idx = [];
  for (let j = 0; j < UP; j++)
    for (let i = 0; i < AROUND; i++) {
      const a = j * (AROUND + 1) + i, b = a + 1, c = a + AROUND + 1, d = c + 1;
      idx.push(a, b, d, a, d, c);
    }
  let base = side;
  for (const flip of [true, false]) {
    const centre = base;
    for (let i = 0; i < AROUND; i++) {
      const p = centre + 1 + i, q = p + 1;
      if (flip) idx.push(centre, q, p);
      else idx.push(centre, p, q);
    }
    base += AROUND + 2;
  }
  return new Uint32Array(idx);
}

{
  const stage = new Stage(document.querySelector("#texture-stage"), { target: [0, 0, 17], distance: 100, yaw: -0.7, pitch: 0.38, fov: 0.5, fit: 0.8 });
  if (stage.gl) {
    stage.spin = still ? 0 : 0.2;
    let pattern = "knurl", depth = 0, want = 0;
    const slider = document.querySelector("#texture-depth");
    const made = grip(pattern, 0);
    const ring = (z) => {
      const pts = [];
      for (let i = 0; i < 120; i++) {
        const a = (i / 120) * 2 * Math.PI, b = ((i + 1) / 120) * 2 * Math.PI;
        pts.push(10 * Math.cos(a), 10 * Math.sin(a), z, 10 * Math.cos(b), 10 * Math.sin(b), z);
      }
      return pts;
    };
    const item = stage.addMesh(
      { pos: made.pos, nrm: made.nrm, idx: gripIndices(), lines: new Float32Array([...ring(0), ...ring(30)]) },
      { color: [0.64, 0.68, 0.74], edges: 0.5 },
    );
    let next = null;
    const buttons = [...document.querySelectorAll("#texture-modes button")];
    for (const b of buttons) {
      b.addEventListener("click", () => {
        buttons.forEach((x) => x.classList.toggle("on", x === b));
        next = b.dataset.pattern;
        want = 0;
      });
    }
    slider.addEventListener("input", () => (want = Number(slider.value)));
    // It presses in once it is seen.
    new IntersectionObserver(([e], obs) => {
      if (!e.isIntersecting) return;
      want = Number(slider.value);
      obs.disconnect();
    }, { threshold: 0.4 }).observe(stage.canvas);
    let shown = -1;
    stage.onFrame = () => {
      depth += (want - depth) * (still ? 1 : 0.12);
      if (next && depth < 0.02) {
        pattern = next;
        next = null;
        want = Number(slider.value);
      }
      if (Math.abs(depth - shown) > 0.002) {
        const g = grip(pattern, depth);
        stage.updateMesh(item, g.pos, g.nrm);
        shown = depth;
      }
    };
  }
}

{
  const stage = new Stage(document.querySelector("#nut-stage"), { target: [15, 8, 8], distance: 64, yaw: -1.25, pitch: 0.5, fov: 0.6, fit: 1.2 });
  if (stage.gl) {
    stage.spin = 0;
    stage.addMesh(models.nut, { clip: [0, -1, 0, 10], edges: 0.45 });
    stage.addMesh(models.nut, { color: [0.31, 0.64, 0.9], alpha: 0.07, edges: 0, clip: [0, 1, 0, -10] });
    stage.onFrame = (time) => {
      if (!stage.drag) stage.cam.yaw = -1.25 + Math.sin(time * 0.35) * 0.3;
    };
  }
}
