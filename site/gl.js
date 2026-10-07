// A small WebGL2 renderer for the site's models: shaded solids with their
// crease outlines, sketch lines that draw themselves, a glow on chosen
// faces, zebra stripes and a section plane. Z is up, millimetres.

const SOLID_VS = `#version 300 es
in vec3 aPos; in vec3 aNrm; in float aFlag; in float aCurv;
uniform mat4 uModel, uViewProj;
uniform vec3 uLo, uSpan, uGrow, uGrowAt;
out vec3 vWorld; out vec3 vNrm; out float vFlag; out float vCurv;
void main() {
  vec3 p = uLo + aPos * uSpan;
  p = uGrowAt + (p - uGrowAt) * uGrow;
  vec4 w = uModel * vec4(p, 1.0);
  vWorld = w.xyz;
  vNrm = mat3(uModel) * aNrm;
  vFlag = aFlag;
  vCurv = aCurv;
  gl_Position = uViewProj * w;
}`;

const SOLID_FS = `#version 300 es
precision highp float;
in vec3 vWorld; in vec3 vNrm; in float vFlag; in float vCurv;
uniform vec3 uColor, uEye, uGlowColor;
uniform float uAlpha, uGlow1, uGlow2, uStripes;
uniform vec4 uClip;
uniform int uMode;
out vec4 frag;
void main() {
  if (dot(vec4(vWorld, 1.0), uClip) > 0.0) discard;
  vec3 n = normalize(vNrm);
  vec3 v = normalize(uEye - vWorld);
  if (!gl_FrontFacing) {
    // The inside, seen through a section: the cut, flat and darker.
    frag = vec4(uColor * 0.42, uAlpha);
    return;
  }
  vec3 col = uColor;
  if (uMode == 1) {
    vec3 r = reflect(-v, n);
    float s = smoothstep(0.42, 0.58, fract((r.x * 0.5 + 0.5) * uStripes));
    col = mix(vec3(0.07, 0.08, 0.1), vec3(0.93), s);
  } else if (uMode == 2) {
    // The curvature map's colours for Gaussian curvature: blue on a
    // saddle, green where flat or unrolling flat, red on a dome.
    // A square-root ramp: gentle curvature shows beside sharp.
    float k = clamp(sign(vCurv) * sqrt(abs(vCurv)) * 1.15, -1.0, 1.0);
    vec3 blue = vec3(0.31, 0.55, 0.95), green = vec3(0.35, 0.82, 0.6), red = vec3(0.93, 0.42, 0.42);
    col = k < 0.0 ? mix(green, blue, -k) : mix(green, red, k);
  }
  vec3 key = normalize(vec3(0.45, -0.55, 0.75));
  vec3 fill = normalize(vec3(-0.6, 0.4, 0.3));
  float diff = max(dot(n, key), 0.0) * 0.62 + max(dot(n, fill), 0.0) * 0.18;
  float hemi = mix(0.26, 0.42, n.z * 0.5 + 0.5);
  float spec = pow(max(dot(reflect(-key, n), v), 0.0), 40.0) * 0.18;
  float rim = pow(1.0 - max(dot(n, v), 0.0), 3.0) * 0.22;
  vec3 lit = col * (hemi + diff) + spec + rim * vec3(0.45, 0.7, 1.0);
  float glow = vFlag > 1.5 ? uGlow2 : (vFlag > 0.5 ? uGlow1 : 0.0);
  lit = mix(lit, uGlowColor * (0.75 + diff), glow);
  frag = vec4(lit, uAlpha);
}`;

const LINE_VS = `#version 300 es
in vec3 aPos; in float aT;
uniform mat4 uModel, uViewProj;
uniform vec3 uLo, uSpan, uGrow, uGrowAt;
out float vT; out vec3 vWorld;
void main() {
  vec3 p = uLo + aPos * uSpan;
  p = uGrowAt + (p - uGrowAt) * uGrow;
  vec4 w = uModel * vec4(p, 1.0);
  vWorld = w.xyz;
  vT = aT;
  gl_Position = uViewProj * w;
  gl_Position.z -= 0.00025 * gl_Position.w;
}`;

const LINE_FS = `#version 300 es
precision highp float;
in float vT; in vec3 vWorld;
uniform vec4 uColor;
uniform float uProgress, uFade;
uniform vec3 uFadeAt;
uniform vec4 uClip;
out vec4 frag;
void main() {
  if (vT > uProgress) discard;
  if (dot(vec4(vWorld, 1.0), uClip) > 0.0) discard;
  float a = uColor.a;
  if (uFade > 0.0) a *= clamp(1.0 - distance(vWorld.xy, uFadeAt.xy) / uFade, 0.0, 1.0);
  frag = vec4(uColor.rgb, a);
}`;

// ---- Matrices (column-major) ------------------------------------------

export const mat4 = {
  identity: () => new Float32Array([1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1]),
  mul(a, b) {
    const o = new Float32Array(16);
    for (let c = 0; c < 4; c++)
      for (let r = 0; r < 4; r++) {
        let s = 0;
        for (let k = 0; k < 4; k++) s += a[k * 4 + r] * b[c * 4 + k];
        o[c * 4 + r] = s;
      }
    return o;
  },
  perspective(fovy, aspect, near, far) {
    const f = 1 / Math.tan(fovy / 2);
    const nf = 1 / (near - far);
    return new Float32Array([f / aspect, 0, 0, 0, 0, f, 0, 0, 0, 0, (far + near) * nf, -1, 0, 0, 2 * far * near * nf, 0]);
  },
  lookAt(eye, at, up) {
    const z = norm(sub(eye, at));
    const x = norm(cross(up, z));
    const y = cross(z, x);
    return new Float32Array([x[0], y[0], z[0], 0, x[1], y[1], z[1], 0, x[2], y[2], z[2], 0, -dot(x, eye), -dot(y, eye), -dot(z, eye), 1]);
  },
  translate: (x, y, z) => new Float32Array([1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, x, y, z, 1]),
  rotZ(a) {
    const c = Math.cos(a), s = Math.sin(a);
    return new Float32Array([c, s, 0, 0, -s, c, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1]);
  },
  rotX(a) {
    const c = Math.cos(a), s = Math.sin(a);
    return new Float32Array([1, 0, 0, 0, 0, c, s, 0, 0, -s, c, 0, 0, 0, 0, 1]);
  },
  rotY(a) {
    const c = Math.cos(a), s = Math.sin(a);
    return new Float32Array([c, 0, -s, 0, 0, 1, 0, 0, s, 0, c, 0, 0, 0, 0, 1]);
  },
};
const sub = (a, b) => [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
const dot = (a, b) => a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
const cross = (a, b) => [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]];
const norm = (a) => {
  const n = Math.hypot(a[0], a[1], a[2]) || 1;
  return [a[0] / n, a[1] / n, a[2] / n];
};
export const ease = (t) => (t <= 0 ? 0 : t >= 1 ? 1 : 1 - Math.pow(1 - t, 3));
export const smooth = (a, b, t) => ease((t - a) / (b - a));

// ---- Models -----------------------------------------------------------

let library = null;
export async function loadModels(base = "assets/") {
  if (!library) {
    library = Promise.all([
      fetch(base + "models.json").then((r) => r.json()),
      fetch(base + "models.bin").then((r) => r.arrayBuffer()),
    ]).then(([manifest, bin]) => {
      const out = {};
      for (const [name, m] of Object.entries(manifest)) {
        const view = (Type, [start, count]) => new Type(bin, start, count);
        const lo = m.lo, span = m.hi.map((h, i) => Math.max(h - lo[i], 1e-9));
        out[name] = {
          lo, hi: m.hi, span,
          pos: view(Uint16Array, m.pos),
          nrm: view(Int8Array, m.nrm),
          flag: view(Uint8Array, m.flag),
          curv: view(Int8Array, m.curv),
          idx: view(m.idx32 ? Uint32Array : Uint16Array, m.idx),
          lines: view(Uint16Array, m.lines),
        };
      }
      return out;
    });
  }
  return library;
}

// ---- A stage: one canvas, a camera, things to draw ----------------------

export class Stage {
  constructor(canvas, { target = [0, 0, 0], distance = 120, yaw = -0.6, pitch = 0.5, fov = 0.5 } = {}) {
    this.canvas = canvas;
    const gl = canvas.getContext("webgl2", { antialias: true, alpha: true, premultipliedAlpha: false });
    this.gl = gl;
    if (!gl) return;
    this.solid = program(gl, SOLID_VS, SOLID_FS);
    this.line = program(gl, LINE_VS, LINE_FS);
    this.items = [];
    this.cam = { target, distance, yaw, pitch, fov };
    this.spin = 0.12;
    this.drag = null;
    this.visible = true;
    this.onFrame = null;
    this.afterFrame = null;
    this.start = performance.now();
    canvas.addEventListener("pointerdown", (e) => {
      this.drag = { x: e.clientX, y: e.clientY, yaw: this.cam.yaw, pitch: this.cam.pitch };
      canvas.setPointerCapture(e.pointerId);
      canvas.classList.add("grabbing");
    });
    canvas.addEventListener("pointermove", (e) => {
      if (!this.drag) return;
      this.cam.yaw = this.drag.yaw - (e.clientX - this.drag.x) * 0.008;
      this.cam.pitch = Math.min(1.45, Math.max(-0.2, this.drag.pitch + (e.clientY - this.drag.y) * 0.006));
    });
    const up = () => {
      this.drag = null;
      canvas.classList.remove("grabbing");
    };
    canvas.addEventListener("pointerup", up);
    canvas.addEventListener("pointercancel", up);
    new IntersectionObserver(([e]) => (this.visible = e.isIntersecting)).observe(canvas);
    const loop = (now) => {
      if (this.visible) this.frame(now);
      requestAnimationFrame(loop);
    };
    requestAnimationFrame(loop);
  }

  // A solid from the library; its draw state lives on the returned item.
  addMesh(mesh, opts = {}) {
    const gl = this.gl;
    const floats = mesh.pos instanceof Float32Array;
    const vao = gl.createVertexArray();
    gl.bindVertexArray(vao);
    const buffers = {
      pos: attrib(gl, this.solid.prog, "aPos", mesh.pos, 3, floats ? gl.FLOAT : gl.UNSIGNED_SHORT, !floats),
      nrm: attrib(gl, this.solid.prog, "aNrm", mesh.nrm, 3, floats ? gl.FLOAT : gl.BYTE, !floats),
    };
    const verts = mesh.pos.length / 3;
    attrib(gl, this.solid.prog, "aFlag", mesh.flag ?? new Uint8Array(verts), 1, gl.UNSIGNED_BYTE, false);
    attrib(gl, this.solid.prog, "aCurv", mesh.curv ?? new Int8Array(verts), 1, gl.BYTE, true);
    const ib = gl.createBuffer();
    gl.bindBuffer(gl.ELEMENT_ARRAY_BUFFER, ib);
    gl.bufferData(gl.ELEMENT_ARRAY_BUFFER, mesh.idx, gl.STATIC_DRAW);
    gl.bindVertexArray(null);
    if (floats) {
      mesh.lo ??= [0, 0, 0];
      mesh.span ??= [1, 1, 1];
    }
    const lines = this.makeLines(mesh.lines ?? new Uint16Array(0), null, mesh, floats);
    const item = {
      kind: "mesh", vao, count: mesh.idx.length, type: mesh.idx instanceof Uint32Array ? gl.UNSIGNED_INT : gl.UNSIGNED_SHORT,
      mesh, lines, model: mat4.identity(), color: [0.62, 0.66, 0.72], alpha: 1, edges: 0.55,
      edgeColor: [0.06, 0.07, 0.09], glow1: 0, glow2: 0, mode: 0, stripes: 9,
      grow: [1, 1, 1], growAt: [0, 0, 0], clip: [0, 0, 0, -1], show: true, buffers, ...opts,
    };
    this.items.push(item);
    return item;
  }

  // New positions and normals for a mesh made here (float arrays of the
  // same length), drawn from the next frame.
  updateMesh(item, pos, nrm) {
    const gl = this.gl;
    gl.bindBuffer(gl.ARRAY_BUFFER, item.buffers.pos);
    gl.bufferSubData(gl.ARRAY_BUFFER, 0, pos);
    gl.bindBuffer(gl.ARRAY_BUFFER, item.buffers.nrm);
    gl.bufferSubData(gl.ARRAY_BUFFER, 0, nrm);
  }

  // Lines from points in mm: pairs for segments; `t` per point (0..1) for
  // the order they draw in.
  addLines(points, opts = {}) {
    const lo = [0, 0, 0], span = [1, 1, 1];
    const item = {
      kind: "lines", ...this.makeLines(new Float32Array(points), opts.t ?? null, { lo, span }, true),
      model: mat4.identity(), color: [0.44, 0.83, 0.79, 1], progress: 1, fade: 0, fadeAt: [0, 0, 0],
      grow: [1, 1, 1], growAt: [0, 0, 0], clip: [0, 0, 0, -1], show: true, ...opts,
    };
    this.items.push(item);
    return item;
  }

  makeLines(data, t, ref, floats = false) {
    const gl = this.gl;
    const vao = gl.createVertexArray();
    gl.bindVertexArray(vao);
    attrib(gl, this.line.prog, "aPos", data, 3, floats ? gl.FLOAT : gl.UNSIGNED_SHORT, !floats);
    const count = data.length / 3;
    attrib(gl, this.line.prog, "aT", t ?? new Float32Array(count), 1, gl.FLOAT, false);
    gl.bindVertexArray(null);
    return { vao, count, lo: floats ? [0, 0, 0] : ref.lo, span: floats ? [1, 1, 1] : ref.span };
  }

  // Where a point in mm lands on the canvas, in CSS pixels.
  project(p) {
    const v = mat4.mul(this.viewProj, new Float32Array([1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, p[0], p[1], p[2], 1]));
    const w = v[15];
    return [((v[12] / w) * 0.5 + 0.5) * this.cssW, (1 - ((v[13] / w) * 0.5 + 0.5)) * this.cssH, v[14] / w];
  }

  frame(now) {
    const gl = this.gl;
    const dpr = Math.min(devicePixelRatio || 1, 2);
    const w = this.canvas.clientWidth, h = this.canvas.clientHeight;
    if (!w || !h) return;
    if (this.canvas.width !== Math.round(w * dpr) || this.canvas.height !== Math.round(h * dpr)) {
      this.canvas.width = Math.round(w * dpr);
      this.canvas.height = Math.round(h * dpr);
    }
    this.cssW = w;
    this.cssH = h;
    const time = (now - this.start) / 1000;
    if (!this.drag && this.spin) this.cam.yaw += this.spin * 0.016;
    if (this.onFrame) this.onFrame(time);
    const c = this.cam;
    const eye = [
      c.target[0] + c.distance * Math.cos(c.pitch) * Math.cos(c.yaw),
      c.target[1] + c.distance * Math.cos(c.pitch) * Math.sin(c.yaw),
      c.target[2] + c.distance * Math.sin(c.pitch),
    ];
    this.eye = eye;
    // Near and far follow the distance, so depth keeps its precision.
    this.viewProj = mat4.mul(mat4.perspective(c.fov, w / h, c.distance * 0.25, c.distance * 6), mat4.lookAt(eye, c.target, [0, 0, 1]));
    gl.viewport(0, 0, this.canvas.width, this.canvas.height);
    gl.clearColor(0, 0, 0, 0);
    gl.clear(gl.COLOR_BUFFER_BIT | gl.DEPTH_BUFFER_BIT);
    gl.enable(gl.DEPTH_TEST);
    gl.enable(gl.BLEND);
    gl.blendFunc(gl.SRC_ALPHA, gl.ONE_MINUS_SRC_ALPHA);
    // Opaque solids, then their outlines, then what is see-through.
    const solids = this.items.filter((i) => i.kind === "mesh" && i.show);
    for (const it of solids.filter((i) => i.alpha >= 1)) this.drawSolid(it);
    for (const it of solids) if (it.edges > 0 && it.alpha > 0.05) this.drawLines(it.lines, it, [...it.edgeColor, it.edges * Math.min(it.alpha * 1.5, 1)]);
    // Overlay lines (sketches, dimensions, a bed) never hide what is
    // behind them: they write no depth, and are skipped while invisible.
    gl.depthMask(false);
    for (const it of this.items.filter((i) => i.kind === "lines" && i.show && i.color[3] > 0.01)) this.drawLines(it, it, it.color);
    gl.depthMask(true);
    gl.depthMask(false);
    for (const it of solids.filter((i) => i.alpha < 1 && i.alpha > 0)) this.drawSolid(it);
    gl.depthMask(true);
    if (this.afterFrame) this.afterFrame(time);
  }

  drawSolid(it) {
    const gl = this.gl, p = this.solid;
    gl.useProgram(p.prog);
    gl.uniformMatrix4fv(p.u.uModel, false, it.model);
    gl.uniformMatrix4fv(p.u.uViewProj, false, this.viewProj);
    gl.uniform3fv(p.u.uLo, it.mesh.lo);
    gl.uniform3fv(p.u.uSpan, it.mesh.span);
    gl.uniform3fv(p.u.uGrow, it.grow);
    gl.uniform3fv(p.u.uGrowAt, it.growAt);
    gl.uniform3fv(p.u.uColor, it.color);
    gl.uniform3fv(p.u.uEye, this.eye);
    gl.uniform3fv(p.u.uGlowColor, [0.31, 0.64, 0.9]);
    gl.uniform1f(p.u.uAlpha, it.alpha);
    gl.uniform1f(p.u.uGlow1, it.glow1);
    gl.uniform1f(p.u.uGlow2, it.glow2);
    gl.uniform1f(p.u.uStripes, it.stripes);
    gl.uniform4fv(p.u.uClip, it.clip);
    gl.uniform1i(p.u.uMode, it.mode);
    gl.enable(gl.POLYGON_OFFSET_FILL);
    gl.polygonOffset(1, 1);
    gl.bindVertexArray(it.vao);
    gl.drawElements(gl.TRIANGLES, it.count, it.type, 0);
    gl.disable(gl.POLYGON_OFFSET_FILL);
  }

  drawLines(l, it, color) {
    const gl = this.gl, p = this.line;
    gl.useProgram(p.prog);
    gl.uniformMatrix4fv(p.u.uModel, false, it.model);
    gl.uniformMatrix4fv(p.u.uViewProj, false, this.viewProj);
    gl.uniform3fv(p.u.uLo, l.lo);
    gl.uniform3fv(p.u.uSpan, l.span);
    gl.uniform3fv(p.u.uGrow, it.grow);
    gl.uniform3fv(p.u.uGrowAt, it.growAt);
    gl.uniform4fv(p.u.uColor, color);
    gl.uniform1f(p.u.uProgress, it.progress ?? 1);
    gl.uniform1f(p.u.uFade, it.fade ?? 0);
    gl.uniform3fv(p.u.uFadeAt, it.fadeAt ?? [0, 0, 0]);
    gl.uniform4fv(p.u.uClip, it.clip);
    gl.bindVertexArray(l.vao);
    gl.drawArrays(gl.LINES, 0, l.count);
  }
}

function program(gl, vs, fs) {
  const make = (type, src) => {
    const s = gl.createShader(type);
    gl.shaderSource(s, src);
    gl.compileShader(s);
    if (!gl.getShaderParameter(s, gl.COMPILE_STATUS)) throw new Error(gl.getShaderInfoLog(s));
    return s;
  };
  const prog = gl.createProgram();
  gl.attachShader(prog, make(gl.VERTEX_SHADER, vs));
  gl.attachShader(prog, make(gl.FRAGMENT_SHADER, fs));
  gl.linkProgram(prog);
  if (!gl.getProgramParameter(prog, gl.LINK_STATUS)) throw new Error(gl.getProgramInfoLog(prog));
  const u = {};
  const n = gl.getProgramParameter(prog, gl.ACTIVE_UNIFORMS);
  for (let i = 0; i < n; i++) {
    const name = gl.getActiveUniform(prog, i).name;
    u[name] = gl.getUniformLocation(prog, name);
  }
  return { prog, u };
}

function attrib(gl, prog, name, data, size, type, normalized) {
  const loc = gl.getAttribLocation(prog, name);
  if (loc < 0) return null;
  const buf = gl.createBuffer();
  gl.bindBuffer(gl.ARRAY_BUFFER, buf);
  gl.bufferData(gl.ARRAY_BUFFER, data, gl.DYNAMIC_DRAW);
  gl.enableVertexAttribArray(loc);
  gl.vertexAttribPointer(loc, size, type, normalized, 0, 0);
  return buf;
}

// Lines of a square grid on z = 0, `step` apart, `half` out from the middle.
export function gridLines(cx, cy, half, step) {
  const pts = [];
  for (let v = -half; v <= half + 1e-6; v += step) {
    pts.push(cx - half, cy + v, 0, cx + half, cy + v, 0);
    pts.push(cx + v, cy - half, 0, cx + v, cy + half, 0);
  }
  return pts;
}

// A polyline's segments as pairs, with each point's share of the length
// for the order it draws in.
export function polyline(points, closed = true) {
  const ring = closed ? [...points, points[0]] : points;
  let total = 0;
  const lengths = [0];
  for (let i = 1; i < ring.length; i++) {
    total += Math.hypot(...sub(ring[i], ring[i - 1]));
    lengths.push(total);
  }
  const pts = [], t = [];
  for (let i = 1; i < ring.length; i++) {
    pts.push(...ring[i - 1], ...ring[i]);
    t.push(lengths[i - 1] / total, lengths[i] / total);
  }
  return { points: pts, t: new Float32Array(t) };
}
