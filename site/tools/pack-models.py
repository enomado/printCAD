#!/usr/bin/env python3
"""Pack STL exports from printCAD into the site's models.bin and models.json.

Each mesh is welded, its normals smoothed across edges flatter than the
crease angle and split at sharper ones, and its crease and boundary edges
kept as outlines. Positions are 16-bit, quantised to the mesh's box;
normals 8-bit. A flag per vertex marks what the page lights up on its own
(the bracket's fillet and holes).

The meshes come from the scenes in tools/scenes, each a script that
builds a part and exports it as STL into /var/tmp/meshes:

    mkdir -p /var/tmp/meshes
    for f in site/tools/scenes/*.lua; do printcad --script "$f"; done
    site/tools/pack-models.py /var/tmp/meshes site/assets
"""
import json
import math
import struct
import sys
from collections import defaultdict

CREASE_DEG = 30.0
SMOOTH_PASSES = 4


def read_stl(path):
    data = open(path, "rb").read()
    count = struct.unpack_from("<I", data, 80)[0]
    tris = []
    for i in range(count):
        off = 84 + i * 50
        v = struct.unpack_from("<12f", data, off)
        tris.append((v[3:6], v[6:9], v[9:12]))
    return tris


def sub(a, b):
    return (a[0] - b[0], a[1] - b[1], a[2] - b[2])


def cross(a, b):
    return (a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0])


def norm(a):
    n = math.sqrt(a[0] ** 2 + a[1] ** 2 + a[2] ** 2) or 1.0
    return (a[0] / n, a[1] / n, a[2] / n)


def dot(a, b):
    return a[0] * b[0] + a[1] * b[1] + a[2] * b[2]


def pack(tris, flag_of=None):
    # Weld corners that share a position.
    key = lambda p: (round(p[0], 4), round(p[1], 4), round(p[2], 4))
    points, index = [], {}
    faces = []
    for t in tris:
        ids = []
        for p in t:
            k = key(p)
            if k not in index:
                index[k] = len(points)
                points.append(p)
            ids.append(index[k])
        if len({*ids}) == 3:
            faces.append(ids)
    fnormals = [norm(cross(sub(points[b], points[a]), sub(points[c], points[a]))) for a, b, c in faces]

    # Edges and the faces on them, for creases and outlines.
    edge_faces = defaultdict(list)
    for f, (a, b, c) in enumerate(faces):
        for u, v in ((a, b), (b, c), (c, a)):
            edge_faces[(min(u, v), max(u, v))].append(f)
    cos_crease = math.cos(math.radians(CREASE_DEG))
    outline = []
    for (u, v), fs in edge_faces.items():
        if len(fs) == 2 and dot(fnormals[fs[0]], fnormals[fs[1]]) < cos_crease:
            outline.append((u, v))
    # An edge with one face is a seam where the other side's triangles
    # meet it at a T: an outline only when some other one-faced edge along
    # the same line, overlapping it, belongs to a face turned away.
    lone = [(u, v, fs[0]) for (u, v), fs in edge_faces.items() if len(fs) == 1]
    groups = defaultdict(list)
    for u, v, f in lone:
        d = norm(sub(points[v], points[u]))
        if (d[0], d[1], d[2]) < (0, 0, 0):
            d = (-d[0], -d[1], -d[2])
        p = points[u]
        along = dot(p, d)
        foot = (p[0] - d[0] * along, p[1] - d[1] * along, p[2] - d[2] * along)
        key = (round(d[0], 2), round(d[1], 2), round(d[2], 2), round(foot[0], 1), round(foot[1], 1), round(foot[2], 1))
        a, b = sorted((dot(points[u], d), dot(points[v], d)))
        groups[key].append((u, v, f, a, b))
    for members in groups.values():
        for u, v, f, a, b in members:
            for u2, v2, f2, a2, b2 in members:
                if f2 == f or min(b, b2) - max(a, a2) <= 1e-4:
                    continue
                if dot(fnormals[f], fnormals[f2]) < cos_crease:
                    outline.append((u, v))
                    break

    # Smooth normals per corner: the faces around the point within the
    # crease angle of this one. A corner with a different normal is a
    # vertex of its own.
    around = defaultdict(list)
    for f, tri in enumerate(faces):
        for p in tri:
            around[p].append(f)
    # Gaussian curvature at each welded point, as the curvature map shows
    # it by default: the angle the faces around it fall short of a full
    # turn, over a third of their area. Positive on a dome, negative on a
    # saddle, none on a plane or what unrolls flat.
    angle_sum = defaultdict(float)
    area_sum = defaultdict(float)
    for a, b, c in faces:
        pa, pb, pc = points[a], points[b], points[c]
        area = 0.5 * math.sqrt(sum(x * x for x in cross(sub(pb, pa), sub(pc, pa))))
        for p0, p1, p2, i in ((pa, pb, pc, a), (pb, pc, pa, b), (pc, pa, pb, c)):
            u, w = norm(sub(p1, p0)), norm(sub(p2, p0))
            angle_sum[i] += math.acos(max(-1.0, min(1.0, dot(u, w))))
            area_sum[i] += area / 3
    gauss = {i: (2 * math.pi - angle_sum[i]) / area_sum[i] if area_sum[i] > 1e-12 else 0.0 for i in angle_sum}
    # A few passes of averaging with the points around: a single point's
    # deficit is noisy where the surface turns from dome to saddle, which
    # draws as a dotted ring rather than a band.
    neighbours = defaultdict(set)
    for a, b, c in faces:
        neighbours[a].update((b, c))
        neighbours[b].update((a, c))
        neighbours[c].update((a, b))
    for _ in range(SMOOTH_PASSES):
        gauss = {i: 0.5 * k + 0.5 * sum(gauss[j] for j in neighbours[i]) / len(neighbours[i]) if neighbours[i] else k
                 for i, k in gauss.items()}
    out_pos, out_nrm, out_flag, out_idx, out_curv = [], [], [], [], []
    made = {}
    for f, tri in enumerate(faces):
        for p in tri:
            n = [0.0, 0.0, 0.0]
            for g in around[p]:
                if dot(fnormals[g], fnormals[f]) >= cos_crease:
                    for k in range(3):
                        n[k] += fnormals[g][k]
            n = norm(n)
            k = (p, round(n[0], 2), round(n[1], 2), round(n[2], 2))
            if k not in made:
                made[k] = len(out_pos)
                out_pos.append(points[p])
                out_nrm.append(n)
                c = [sum(points[q][i] for q in tri) / 3 for i in range(3)]
                out_flag.append(flag_of(c, fnormals[f]) if flag_of else 0)
                out_curv.append(gauss.get(p, 0.0))
            out_idx.append(made[k])
    # Outline edges refer to welded points; give each its own two vertices.
    lines = []
    for u, v in outline:
        lines.extend([points[u], points[v]])

    lo = [min(p[i] for p in points) for i in range(3)]
    hi = [max(p[i] for p in points) for i in range(3)]
    span = [max(hi[i] - lo[i], 1e-9) for i in range(3)]
    q = lambda p: [round((p[i] - lo[i]) / span[i] * 65535) for i in range(3)]
    # Scaled by most of the surface, so the few sharp rims (where the
    # deficit is a crease's, not a curvature) do not wash the rest out.
    mags = sorted(abs(k) for k in out_curv)
    scale = mags[int(len(mags) * 0.8)] if mags else 1.0
    scale = scale or 1.0
    curv = [max(-127, min(127, round(k / scale * 127))) for k in out_curv]
    return {
        "lo": lo, "hi": hi, "curv": curv, "curvScale": scale,
        "pos": [c for p in out_pos for c in q(p)],
        "nrm": [round(c * 127) for n in out_nrm for c in n],
        "flag": out_flag,
        "idx": out_idx,
        "lines": [c for p in lines for c in q(p)],
        "tris": len(faces),
    }


def bracket_flag(c, n):
    # The fillet: the round in the inside corner, its axis along Y at
    # x = z = 9 (wall 5 thick, radius 4).
    if 5 - 0.1 <= c[0] <= 9.1 and 5 - 0.1 <= c[2] <= 9.1 and abs(n[1]) < 0.2:
        if abs(math.hypot(c[0] - 9, c[2] - 9) - 4) < 0.3:
            return 1
    # The holes: walls of radius 2.5 bores.
    for cx, cy in ((27, 8), (27, 22)):  # down through the foot
        if abs(math.hypot(c[0] - cx, c[1] - cy) - 2.5) < 0.3 and c[2] <= 5.1:
            return 2
    if abs(math.hypot(c[1] - 15, c[2] - 20) - 2.5) < 0.3 and c[0] <= 5.1:
        return 2
    return 0


def main():
    src, out = sys.argv[1], sys.argv[2]
    names = ["pad", "bracket", "plate", "gear", "sprocket", "nut", "vase"]
    blob = bytearray()
    manifest = {}

    def put(fmt, values):
        start = len(blob)
        blob.extend(struct.pack(f"<{len(values)}{fmt}", *values))
        while len(blob) % 4:
            blob.append(0)
        return [start, len(values)]

    for name in names:
        m = pack(read_stl(f"{src}/{name}.stl"), bracket_flag if name in ("bracket",) else None)
        big = len(m["pos"]) // 3 > 65535
        manifest[name] = {
            "lo": m["lo"], "hi": m["hi"], "tris": m["tris"],
            "pos": put("H", m["pos"]),
            "nrm": put("b", m["nrm"]),
            "flag": put("B", m["flag"]),
            "curv": put("b", m["curv"]),
            "curvScale": m["curvScale"],
            "idx": put("I" if big else "H", m["idx"]),
            "idx32": big,
            "lines": put("H", m["lines"]),
        }
        print(f"{name}: {m['tris']} triangles, {len(m['pos']) // 3} vertices, {len(m['lines']) // 6} edges")
    open(f"{out}/models.bin", "wb").write(blob)
    json.dump(manifest, open(f"{out}/models.json", "w"))
    print(f"models.bin {len(blob) / 1024:.0f} KB")


main()
