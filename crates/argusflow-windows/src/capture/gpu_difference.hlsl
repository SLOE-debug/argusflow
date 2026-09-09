// One 32x32 tile per group; 64 lanes each inspect a 4x4 patch exactly.
StructuredBuffer<uint> Before : register(t0);
StructuredBuffer<uint> After : register(t1);
RWStructuredBuffer<uint4> Bounds : register(u0);
cbuffer Geometry : register(b0) { uint Width; uint Height; uint Columns; uint Mask; };
groupshared uint Left;
groupshared uint Top;
groupshared uint Right;
groupshared uint Bottom;
[numthreads(8, 8, 1)]
void main(uint3 group : SV_GroupID, uint3 lane : SV_GroupThreadID, uint index : SV_GroupIndex) {
    if (index == 0) { Left = Width; Top = Height; Right = 0; Bottom = 0; }
    GroupMemoryBarrierWithGroupSync();
    uint2 origin = group.xy * 32 + lane.xy * 4;
    for (uint dy = 0; dy < 4; dy++) {
        for (uint dx = 0; dx < 4; dx++) {
            uint2 p = origin + uint2(dx, dy);
            if (p.x < Width && p.y < Height) {
                uint offset = p.y * Width + p.x;
                if (((Before[offset] ^ After[offset]) & Mask) != 0) {
                    InterlockedMin(Left, p.x); InterlockedMin(Top, p.y);
                    InterlockedMax(Right, p.x + 1); InterlockedMax(Bottom, p.y + 1);
                }
            }
        }
    }
    GroupMemoryBarrierWithGroupSync();
    if (index == 0) Bounds[group.y * Columns + group.x] = uint4(Left, Top, Right, Bottom);
}
