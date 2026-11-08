# Local scanout packet

DHP2 is a synthetic local fixture/defensive-decoder format, not a live privileged-worker or network video protocol. Raw framebuffer storage must stay inside the trusted capture boundary. The live worker composes internally and returns only visible DHF1 pixels. Matching before/after metadata is best-effort and cannot establish an immutable/coherent physical snapshot. Every integer below is little-endian; the fixture consumer withholds publication until complete decode/composition succeeds. No pointer, file path or arbitrary offset is transferred.

Header24bytes: magic DHP2[0..4], version u16=1[4], header-size u16=24[6], outputwidth u32[8], outputheight u32[12], plane-count u32[16], sum of pixel-byte counts u32[20]. Count1..32; output edges1..16384 and at most128MiB RGBA. Total source bytes at most128MiB. Whole packet length must exactly equal24+64*count+sum.

Each64byte record immediately precedes its pixels:

| Offset | Field |
|---|---|
|0,4|id u32, unique z i32|
|8,9,10,11|kind u8(primary0,overlay1,cursor2); format u8(RGBA0,BGRA1,BGRX2); blend u8(None0,Coverage1,Premultiplied2); clockwise rotation u8(0,90,180,270 encoded0..3)|
|12,14|reflection u16(horizontal1,vertical2; other bits refused), global opacity u16|
|16,20,24|source width,height,stride u32|
|28,32,36,40|integer source crop x,y,width,height u32|
|44,48,52,56|signed destination x,y i32 and width,height u32|
|60|exact pixel-byte count u32=stride*sourceheight|

Reflection acts in source coordinates before clockwise rotation. Scaling/fractional source crops, unknown format/blend/transform semantics, ambiguous IDs/z ordering and an incomplete/nonopaque primary are refused. Native DRM enums must be explicitly translated; their numeric blend/rotation encodings are not this packet's encodings.

DHF1 is the12byte mode-query reply or a visible-only composed RGBA result. The parent reads/validates the fixed header first, fallibly reserves the exact bounded declared size, reads exactly that size and rejects trailing data. An invalid header is rejected before reading its irrelevant body; the failed child is terminated/reaped. Neither format proves installed authorization or coherent physical GPU support. Actual qualifying target tests are still required.
