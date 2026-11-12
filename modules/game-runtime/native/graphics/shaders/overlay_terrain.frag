#version 450
layout(binding=0) uniform sampler2D atlas;
layout(push_constant) uniform Terrain { vec4 rect; vec4 view; vec4 orientation; vec4 map; vec4 fog; vec4 mode; vec4 geometry; vec4 origin; } p;
layout(location=0) in vec2 uv;
layout(location=0) out vec4 color;
vec3 sampleBilinear(vec2 position, ivec2 size, int rowOffset) {
    ivec2 first = ivec2(floor(position));
    ivec2 second = min(first + 1, size - 1);
    precise vec2 fraction = position - vec2(first);
    // Hardware filtering rounds subtexel weights enough to cross the legacy colour bins.
    precise vec3 top = texelFetch(atlas,first+ivec2(0,rowOffset),0).rgb*(1-fraction.x) +
        texelFetch(atlas,ivec2(second.x,first.y+rowOffset),0).rgb*fraction.x;
    precise vec3 bottom = texelFetch(atlas,ivec2(first.x,second.y+rowOffset),0).rgb*(1-fraction.x) +
        texelFetch(atlas,second+ivec2(0,rowOffset),0).rgb*fraction.x;
    precise vec3 value = top*(1-fraction.y) + bottom*fraction.y;
    return value;
}
vec3 encode(vec3 c) {
    if (p.mode.z == 0) return c;
    c = mix(pow((c + .055) / 1.055, vec3(2.4)), c / 12.92, lessThanEqual(c,vec3(.04045)));
    if (p.mode.z == 1) return c * (203.0/80.0);
    c = vec3(dot(c, vec3(.627404,.329283,.043313)),
             dot(c, vec3(.069097,.919540,.011362)), dot(c,vec3(.016391,.088013,.895595)));
    vec3 y = pow(max(c,vec3(0)) * (203.0/10000.0),vec3(2610.0/16384.0));
    return pow((3424.0/4096.0+(2413.0/128.0)*y)/(1+(2392.0/128.0)*y),vec3(2523.0/32.0));
}
// Find the last rounded cell edge at or before this framebuffer pixel.
float cellAt(float pixel, float origin, float center, float side, float step) {
    int lo = 0, hi = int(ceil(side / step));
    while (lo + 1 < hi) {
        int mid = (lo + hi) / 2;
        precise float edge = center + ((origin + float(mid) * step) - center) * p.geometry.z;
        float rounded = sign(edge) * floor(abs(edge) + .5);
        if (rounded <= pixel) lo = mid; else hi = mid;
    }
    return float(lo) * step;
}
void main() {
    float radius = p.view.w, step = p.orientation.z;
    vec2 cell;
    if (p.geometry.z == 1)
        cell = floor(floor(uv*(2*radius+1))/step)*step;
    else
        cell = vec2(cellAt(floor(gl_FragCoord.x),p.origin.x,p.geometry.x,2*radius+1,step),
                    cellAt(floor(gl_FragCoord.y),p.origin.y,p.geometry.y,2*radius+1,step));
    vec2 local = cell - radius + floor(min(vec2(step),vec2(2*radius+1)-cell)/2);
    float distanceSq = dot(local,local);
    if (p.mode.w == 0 && distanceSq > radius*radius) discard;
    precise vec2 north = vec2(p.orientation.x*local.x-p.orientation.y*local.y,
                      p.orientation.y*local.x+p.orientation.x*local.y);
    precise vec2 world = p.view.xy + vec2(north.x,-north.y)*p.view.z;
    precise vec2 mapUv = clamp(vec2(world.x/10240.0,1-world.y/10240.0),vec2(0),vec2(1));
    precise vec3 c = sampleBilinear(mapUv*(p.map.xy-1),ivec2(p.map.xy),0);
    c = clamp((c-.42)*vec3(1.20,1.22,1.16)+.42,vec3(0),vec3(1));
    precise float strength = p.orientation.w/100, lift = strength*.62, style = min(1,strength/.55);
    c = (c+(1-c)*lift)*vec3(1,1-.035*style,1-.115*style);
    precise float distance = sqrt(distanceSq)/radius;
    precise float vignette = (1.02-.01*style)-(.34-.24*style)*distance*distance;
    c = clamp(c*vignette+.004,vec3(0),vec3(1));
    if (p.mode.x != 0) {
        precise float revealed = 0;
        if (p.mode.y != 0 && all(greaterThanEqual(world,vec2(0))) && all(lessThanEqual(world,p.fog.zw))) {
            precise vec2 f = clamp(vec2(world.x,p.fog.w-world.y)*.125-.5,vec2(0),p.fog.xy-1);
            revealed = sampleBilinear(f,ivec2(p.fog.xy),int(p.map.y)).r;
        }
        c = vec3(.30,.31,.28)+(c-vec3(.30,.31,.28))*revealed;
    }
    float levels = p.orientation.w == 0 ? 34 : 48;
    c = floor(clamp(c,vec3(0),vec3(1))*levels+.5)/levels;
    color = vec4(encode(c),1);
}
