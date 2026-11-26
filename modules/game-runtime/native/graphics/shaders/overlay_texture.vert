#version 450
layout(push_constant) uniform Draw { vec4 rect; vec4 uvRect; vec4 rotationViewport; vec4 clip; vec4 tint; vec4 mode; vec4 unused1; vec4 unused2; } p;
layout(location=0) out vec2 uv;
void main() {
    const vec2 corners[6]=vec2[6](vec2(0,0),vec2(1,0),vec2(0,1),vec2(0,1),vec2(1,0),vec2(1,1));
    vec2 q=corners[gl_VertexIndex]; uv=mix(p.uvRect.xy,p.uvRect.zw,q);
    vec2 local=(q-.5)*(p.rect.zw-p.rect.xy);
    vec2 pixel=(p.rect.xy+p.rect.zw)*.5+vec2(p.rotationViewport.x*local.x-p.rotationViewport.y*local.y,p.rotationViewport.y*local.x+p.rotationViewport.x*local.y);
    gl_Position=vec4(pixel*2/p.rotationViewport.zw-1,0,1);
}
