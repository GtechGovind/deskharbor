#pragma once
#include <stdint.h>
#include <stddef.h>
#include <string.h>
#if __has_include(<drm/drm_fourcc.h>)
#include <drm/drm_fourcc.h>
#include <drm/drm_mode.h>
#else
#include <libdrm/drm_fourcc.h>
#include <libdrm/drm_mode.h>
#endif

enum result {
    OK = 0, BAD_REQUEST = 2, PERMISSION, NO_SCANOUT, COMPOSITION, GEOMETRY,
    MODIFIER, PIXEL_FORMAT, INVALID_BUFFER, UNAVAILABLE, SANDBOX, CHANGED
};
struct state {
    uint32_t crtc, fb, width, height, planes;
    uint64_t src_x, src_y, src_w, src_h, x, y, w, h, rotation, type, alpha;
};

static inline int unsupported_screen_effect(const char *name, uint64_t value) {
    return value && (!strcmp(name, "CTM") || !strcmp(name, "GAMMA_LUT") ||
        !strcmp(name, "DEGAMMA_LUT") || !strcmp(name, "HDR_OUTPUT_METADATA") ||
        !strcmp(name, "Content Protection") || !strcmp(name, "Colorspace") ||
        !strcmp(name, "panel orientation") || !strcmp(name, "underscan") ||
        !strcmp(name, "underscan hborder") || !strcmp(name, "underscan vborder") ||
        !strcmp(name, "scaling mode"));
}

static inline int eligible(const struct state *s) {
    if (!s->planes || !s->width || !s->height || !s->crtc || !s->fb) return NO_SCANOUT;
    if (s->planes != 1 || s->type != 1 || s->alpha != 65535) return COMPOSITION;
    if (s->rotation != 1 || s->x || s->y || s->src_x || s->src_y ||
        s->w != s->width || s->h != s->height ||
        s->src_w != (uint64_t)s->width << 16 || s->src_h != (uint64_t)s->height << 16)
        return GEOMETRY;
    return OK;
}
#define CAPTURE_LIMIT (128u * 1024u * 1024u)

static inline int extent(const struct drm_mode_fb_cmd2 *fb, const struct state *s, size_t *end) {
    if (!(fb->flags & DRM_MODE_FB_MODIFIERS) || fb->modifier[0] != DRM_FORMAT_MOD_LINEAR)
        return MODIFIER;
    if (fb->pixel_format != DRM_FORMAT_XRGB8888 || fb->pitches[1] || fb->pitches[2] || fb->pitches[3])
        return PIXEL_FORMAT;
    if (fb->width != s->width || fb->height != s->height || (fb->flags & DRM_MODE_FB_INTERLACED))
        return GEOMETRY;
    if (!fb->width || !fb->height || fb->width > 16384 || fb->height > 16384 ||
        fb->pitches[0] < (uint64_t)fb->width * 4)
        return INVALID_BUFFER;
    uint64_t last = fb->offsets[0] + (uint64_t)(fb->height - 1) * fb->pitches[0] + (uint64_t)fb->width * 4;
    if (last > CAPTURE_LIMIT) return INVALID_BUFFER;
    *end = (size_t)last;
    return OK;
}
static inline void unpack(const struct drm_mode_fb_cmd2 *fb, const uint8_t *memory, uint8_t *out) {
    for (uint32_t y = 0; y < fb->height; ++y) {
        const uint8_t *row = memory + fb->offsets[0] + (size_t)y * fb->pitches[0];
        for (uint32_t x = 0; x < fb->width; ++x, out += 4, row += 4) {
            out[0] = row[2];
            out[1] = row[1];
            out[2] = row[0];
            out[3] = 255;
        }
    }
}
