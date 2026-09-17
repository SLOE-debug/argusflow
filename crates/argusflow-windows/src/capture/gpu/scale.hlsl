Texture2D<float4> desktop : register(t0);
RWStructuredBuffer<uint> output_pixels : register(u0);
cbuffer Geometry : register(b0) {
    uint raw_width; uint raw_height; uint width; uint height;
    uint rotation; uint pad0; uint pad1; uint pad2;
};
[numthreads(16,16,1)]
void main(uint3 id : SV_DispatchThreadID) {
    if (id.x >= width || id.y >= height) return;
    float2 uv = (float2(id.xy) + 0.5) / float2(width,height);
    if (rotation == 1) uv = float2(uv.y, 1.0-uv.x);
    else if (rotation == 2) uv = 1.0-uv;
    else if (rotation == 3) uv = float2(1.0-uv.y, uv.x);
    float2 p = uv * float2(raw_width,raw_height) - 0.5;
    int2 lo = int2(floor(p));
    float2 f = frac(p);
    int2 extent = int2(raw_width-1,raw_height-1);
    float4 a = desktop.Load(int3(clamp(lo,int2(0,0),extent),0));
    float4 b = desktop.Load(int3(clamp(lo+int2(1,0),int2(0,0),extent),0));
    float4 c = desktop.Load(int3(clamp(lo+int2(0,1),int2(0,0),extent),0));
    float4 d = desktop.Load(int3(clamp(lo+int2(1,1),int2(0,0),extent),0));
    uint3 rgb = uint3(round(saturate(lerp(lerp(a,b,f.x),lerp(c,d,f.x),f.y).rgb)*255.0));
    output_pixels[id.y*width+id.x] = rgb.b | (rgb.g<<8) | (rgb.r<<16) | 0xff000000;
}
