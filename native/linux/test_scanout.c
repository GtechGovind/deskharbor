#include "scanout.h"
#include <stdio.h>
#include <string.h>

static int failures;
#define CHECK(expr) do { if (!(expr)) { fprintf(stderr, "failed line %d: %s\n", __LINE__, #expr); failures++; } } while (0)

int main(void) {
    CHECK(!unsupported_screen_effect("panel orientation", 0));
    CHECK(unsupported_screen_effect("panel orientation", 1));
    CHECK(unsupported_screen_effect("underscan", 2));
    CHECK(unsupported_screen_effect("underscan hborder", 8));
    CHECK(unsupported_screen_effect("underscan vborder", 4));
    struct state s = {.crtc=7, .fb=10, .width=2, .height=2, .planes=1,
        .src_w=131072, .src_h=131072, .w=2, .h=2, .rotation=1, .type=1, .alpha=65535};
    CHECK(eligible(&s) == OK);
    s.planes = 0; CHECK(eligible(&s) == NO_SCANOUT);
    s.planes = 2; CHECK(eligible(&s) == COMPOSITION);
    s.planes = 1; s.type = 2; CHECK(eligible(&s) == COMPOSITION);
    s.type = 1; s.rotation = 2; CHECK(eligible(&s) == GEOMETRY);
    s.rotation = 1; s.src_x = 1; CHECK(eligible(&s) == GEOMETRY);
    s.src_x = 0; s.h = 1; CHECK(eligible(&s) == GEOMETRY);
    s.h = 2; s.alpha = 32768; CHECK(eligible(&s) == COMPOSITION);
    s.alpha = 65535;

    struct drm_mode_fb_cmd2 fb = {.width=2,.height=2,.pixel_format=DRM_FORMAT_XRGB8888,
        .flags=DRM_MODE_FB_MODIFIERS,.offsets={2},.pitches={10}};
    size_t end = 0;
    CHECK(extent(&fb, &s, &end) == OK && end == 20);
    uint8_t source[] = {99,99, 3,2,1,0, 6,5,4,0, 77,77, 9,8,7,0, 12,11,10,0};
    uint8_t rgba[16] = {0};
    uint8_t expected[] = {1,2,3,255, 4,5,6,255, 7,8,9,255, 10,11,12,255};
    unpack(&fb, source, rgba);
    CHECK(memcmp(rgba, expected, 16) == 0);
    fb.modifier[0] = 1; CHECK(extent(&fb, &s, &end) == MODIFIER);
    fb.flags = 0; fb.modifier[0] = 0; CHECK(extent(&fb, &s, &end) == MODIFIER);
    fb.flags = DRM_MODE_FB_MODIFIERS; fb.pitches[0] = 7; CHECK(extent(&fb, &s, &end) == INVALID_BUFFER);
    fb.pitches[0] = 10; fb.pitches[1] = 2; CHECK(extent(&fb, &s, &end) == PIXEL_FORMAT);
    fb.pitches[1] = 0; fb.width = 3; CHECK(extent(&fb, &s, &end) == GEOMETRY);
    fb.width = 2; fb.pixel_format = DRM_FORMAT_NV12; CHECK(extent(&fb, &s, &end) == PIXEL_FORMAT);
    fb.pixel_format = DRM_FORMAT_XRGB8888; fb.offsets[0] = UINT32_MAX;
    CHECK(extent(&fb, &s, &end) == INVALID_BUFFER);
    return failures != 0;
}
