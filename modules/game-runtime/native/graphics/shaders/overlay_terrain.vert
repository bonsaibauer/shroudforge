#version 450
layout(push_constant) uniform Terrain { vec4 rect; vec4 view; vec4 orientation; vec4 map; vec4 fog; vec4 mode; vec4 geometry; vec4 origin; } p;
layout(location=0) out vec2 uv;
void main() {
    const vec2 corners[6] = vec2[6](vec2(0,0),vec2(1,0),vec2(0,1),vec2(0,1),vec2(1,0),vec2(1,1));
    uv = corners[gl_VertexIndex];
    gl_Position = vec4(mix(p.rect.xy, p.rect.zw, uv), 0, 1);
}
