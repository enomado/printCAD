#version 450

// A square patch of a plane, made from the vertex index alone (no vertex
// buffer): two triangles around `center`, `radius` out along u and v.

layout(push_constant) uniform Grid {
    mat4 view_proj;
    vec4 camera_step;     // xyz eye, w finest spacing
    vec4 clip_plane;
    vec4 anchor_radius;   // xyz a grid node near the patch, w patch radius
    vec4 u_minor;         // xyz unit u, w strength of the finest lines
    vec4 v_major;         // xyz unit v, w strength of lines far apart
    vec4 center_axis;     // xyz patch centre, w strength of the axes
    vec4 forward_near;    // xyz view forward, w depth where the view starts
    vec4 color_far;       // rgb lines, w depth where the view ends
    vec4 u_axis_color;    // rgb the u axis, w where the v axis crosses u
    vec4 v_axis_color;    // rgb the v axis, w where the u axis crosses v
} g;

layout(location = 0) out vec2 out_plane;
layout(location = 1) out vec3 out_world;

out gl_PerVertex {
    vec4 gl_Position;
    float gl_ClipDistance[1];
};

const vec2 CORNERS[6] = vec2[](
    vec2(-1.0, -1.0), vec2(1.0, -1.0), vec2(1.0, 1.0),
    vec2(-1.0, -1.0), vec2(1.0, 1.0), vec2(-1.0, 1.0)
);

void main() {
    vec2 corner = CORNERS[gl_VertexIndex] * g.anchor_radius.w;
    vec3 world = g.center_axis.xyz + g.u_minor.xyz * corner.x + g.v_major.xyz * corner.y;
    vec3 rel = world - g.anchor_radius.xyz;
    out_plane = vec2(dot(rel, g.u_minor.xyz), dot(rel, g.v_major.xyz));
    out_world = world;
    vec4 clip = g.view_proj * vec4(world, 1.0);
    // A hair away from the eye, so a face lying in the plane wins the
    // depth test over the lines.
    clip.z += 2.0e-5 * clip.w;
    gl_Position = clip;
    gl_ClipDistance[0] = dot(g.clip_plane.xyz, world) + g.clip_plane.w;
}
