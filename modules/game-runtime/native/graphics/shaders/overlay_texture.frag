#version 450
layout(binding=0) uniform sampler2D image;
layout(push_constant) uniform Draw { vec4 rect; vec4 uvRect; vec4 rotationViewport; vec4 clip; vec4 tint; vec4 mode; vec4 unused1; vec4 unused2; } p;
layout(location=0) in vec2 uv;
layout(location=0) out vec4 color;
vec3 encode(vec3 c) {
    if(p.mode.x==0) return c;
    c=mix(pow((c+.055)/1.055,vec3(2.4)),c/12.92,lessThanEqual(c,vec3(.04045)));
    if(p.mode.x==1) return c*(203.0/80.0);
    c=vec3(dot(c,vec3(.627404,.329283,.043313)),dot(c,vec3(.069097,.919540,.011362)),dot(c,vec3(.016391,.088013,.895595)));
    vec3 y=pow(max(c,vec3(0))*(203.0/10000.0),vec3(2610.0/16384.0));
    return pow((3424.0/4096.0+(2413.0/128.0)*y)/(1+(2392.0/128.0)*y),vec3(2523.0/32.0));
}
void main() {
    vec2 clipOffset=abs(gl_FragCoord.xy-p.clip.xy);
    if(p.clip.z>0 && (p.clip.w!=0 ? max(clipOffset.x,clipOffset.y) : length(clipOffset))>p.clip.z) discard;

    vec4 sampleColor=texture(image,uv)*p.tint;
    if(sampleColor.a==0) discard;
    color=vec4(encode(sampleColor.rgb),sampleColor.a);
}
