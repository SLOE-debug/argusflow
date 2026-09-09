// 每组处理一个候选块，逐像素 Load；不插值、不按比例忽略细小变化。
Texture2D<float4> Before : register(t0);
Texture2D<float4> After : register(t1);
StructuredBuffer<uint4> Locations : register(t2);
RWStructuredBuffer<uint4> Result : register(u0);
cbuffer Geometry : register(b0) { uint Width; uint Height; uint Count; uint Reserved; };
groupshared uint Left;
groupshared uint Top;
groupshared uint Right;
groupshared uint Bottom;
groupshared uint Changed;
[numthreads(8, 8, 1)]
void main(uint3 group : SV_GroupID, uint3 lane : SV_GroupThreadID, uint index : SV_GroupIndex) {
    uint4 entry = Locations[group.x];
    if (index == 0) { Left = Width; Top = Height; Right = 0; Bottom = 0; Changed = 0; }
    GroupMemoryBarrierWithGroupSync();
    for (uint dy = 0; dy < 4; dy++) {
        for (uint dx = 0; dx < 4; dx++) {
            uint2 local = lane.xy * 4 + uint2(dx,dy);
            uint2 p = entry.xy + local;
            if (p.x < Width && p.y < Height) {
                // BGRA UNORM 的 Load 映射是逐字节唯一的；X 通道从不参与颜色比较。
                if (any(Before.Load(int3(p,0)).rgb != After.Load(int3(entry.zw + local,0)).rgb)) {
                    InterlockedMin(Left,p.x); InterlockedMin(Top,p.y);
                    InterlockedMax(Right,p.x+1); InterlockedMax(Bottom,p.y+1);
                    InterlockedAdd(Changed,1);
                }
            }
        }
    }
    GroupMemoryBarrierWithGroupSync();
    if (index == 0) {
        Result[group.x*2] = uint4(Left,Top,Right,Bottom);
        Result[group.x*2+1] = uint4(Changed,min(32,Width-entry.x)*min(32,Height-entry.y),0,0);
    }
}
