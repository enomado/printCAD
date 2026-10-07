#version 450

// Three decades of lines at once (the finest spacing, ten and a hundred
// times it), each as strong as its spacing on screen at this fragment
// allows: crowded lines fade out before they blur, lines far apart show
// at full strength. The strength depends on screen spacing alone, so the
// grid thins with distance and zoom and does not jump when the host
// changes the finest spacing by a decade.

layout(push_constant) uniform Grid {
    mat4 view_proj;
    vec4 camera_step;
    vec4 clip_plane;
    vec4 anchor_radius;
    vec4 u_minor;
    vec4 v_major;
    vec4 center_axis;
    vec4 forward_near;
    vec4 color_far;
    vec4 u_axis_color;
    vec4 v_axis_color;
} g;

layout(location = 0) in vec2 in_plane;
layout(location = 1) in vec3 in_world;

layout(location = 0) out vec4 out_color;

/// How strongly lines `px` apart on screen show.
float strength(float px) {
    float minor = g.u_minor.w;
    float major = g.v_major.w;
    return minor * smoothstep(3.0, 14.0, px) + (major - minor) * smoothstep(40.0, 160.0, px);
}

/// How much of this pixel the lines `spacing` apart cover (about one pixel
/// wide), and how far apart they stand on screen.
float coverage(float spacing, out float px) {
    vec2 c = in_plane / spacing;
    vec2 d = max(fwidth(c), vec2(1.0e-6));
    px = 1.0 / max(d.x, d.y);
    vec2 off = abs(fract(c - 0.5) - 0.5) / d;
    return 1.0 - min(min(off.x, off.y), 1.0);
}

/// How much of this pixel a line `width` pixels wide at plane coordinate
/// `at` covers, where `value` is this fragment's coordinate across it.
float axis_line(float value, float at, float width) {
    float d = max(fwidth(value), 1.0e-9);
    return clamp(0.5 * width + 0.5 - abs(value - at) / d, 0.0, 1.0);
}

void main() {
    float alpha = 0.0;
    float spacing = g.camera_step.w;
    for (int level = 0; level < 3; ++level) {
        float px;
        float cover = coverage(spacing, px);
        alpha = max(alpha, cover * strength(px));
        spacing *= 10.0;
    }
    vec3 color = g.color_far.rgb;

    // The axes through the origin, in their own colours over the lines.
    float axis_alpha = g.center_axis.w;
    float on_u = axis_line(in_plane.y, g.v_axis_color.w, 1.5) * axis_alpha;
    float on_v = axis_line(in_plane.x, g.u_axis_color.w, 1.5) * axis_alpha;
    if (on_u > 0.0 || on_v > 0.0) {
        vec3 axis = on_u >= on_v ? g.u_axis_color.rgb : g.v_axis_color.rgb;
        float a = max(on_u, on_v);
        color = mix(color, axis, a / max(max(a, alpha), 1.0e-6));
        alpha = max(alpha, a);
    }

    // Out toward the patch's rim, and before the view's depth range would
    // cut it off.
    float r = length(in_world - g.center_axis.xyz) / g.anchor_radius.w;
    float fade = 1.0 - smoothstep(0.45, 1.0, r);
    float near = g.forward_near.w;
    float far = g.color_far.w;
    float depth = dot(in_world - g.camera_step.xyz, g.forward_near.xyz);
    float span = max(far - near, 1.0e-6);
    fade *= 1.0 - smoothstep(near + 0.8 * span, far, depth);

    alpha *= fade;
    if (alpha < 1.0 / 255.0) {
        discard;
    }
    out_color = vec4(color, alpha);
}
