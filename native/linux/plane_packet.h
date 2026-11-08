#pragma once
#include "scanout.h"
#include <limits.h>

#define DH_MAX_PLANES 32u
#define DH_HEADER_BYTES 24u
#define DH_RECORD_BYTES 64u
struct dh_plane {
    struct state state;
    uint32_t id;
    uint64_t z, blend;
    unsigned has_z, has_blend;
};

static inline int dh_plane_property(const char *name, uint64_t value) {
    const char *known[]={"FB_ID","CRTC_ID","SRC_X","SRC_Y","SRC_W","SRC_H","CRTC_X","CRTC_Y","CRTC_W","CRTC_H",
        "type","rotation","alpha","zpos","pixel blend mode","IN_FORMATS","IN_FENCE_FD","FB_DAMAGE_CLIPS","SIZE_HINTS"};
    for(unsigned i=0;i<sizeof known/sizeof *known;i++)if(!strcmp(name,known[i]))return OK;
    if(!strcmp(name,"HOTSPOT_X") || !strcmp(name,"HOTSPOT_Y"))return value?GEOMETRY:OK;
    if(!strcmp(name,"COLOR_PIPELINE") || !strcmp(name,"COLOR_ENCODING") || !strcmp(name,"COLOR_RANGE"))return value?COMPOSITION:OK;
    return COMPOSITION;
}

static inline int dh_source_extent(const struct drm_mode_fb_cmd2 *fb, size_t *end) {
    if (!fb->width || !fb->height || fb->width>16384 || fb->height>16384) return INVALID_BUFFER;
    if (fb->pixel_format!=DRM_FORMAT_XRGB8888 && fb->pixel_format!=DRM_FORMAT_ARGB8888) return PIXEL_FORMAT;
    if (fb->flags!=DRM_MODE_FB_MODIFIERS || fb->modifier[0]!=DRM_FORMAT_MOD_LINEAR) return MODIFIER;
    for (unsigned i=1;i<4;i++) if (fb->handles[i] || fb->pitches[i] || fb->offsets[i] || fb->modifier[i]) return PIXEL_FORMAT;
    uint64_t minimum=(uint64_t)fb->width*4;
    uint64_t last=(uint64_t)fb->offsets[0]+(uint64_t)fb->pitches[0]*(fb->height-1)+minimum;
    if (fb->pitches[0]<minimum || last>CAPTURE_LIMIT || (uint64_t)fb->width*fb->height*4>CAPTURE_LIMIT) return INVALID_BUFFER;
    *end=(size_t)last; return OK;
}
static inline void dh_u16(uint8_t *out, uint16_t value) { out[0]=(uint8_t)value;out[1]=(uint8_t)(value>>8); }
static inline void dh_u32(uint8_t *out, uint32_t value) { for(unsigned i=0;i<4;i++)out[i]=(uint8_t)(value>>(i*8)); }
static inline int dh_record(const struct dh_plane *plane, const struct drm_mode_fb_cmd2 *fb,
    uint32_t width, uint32_t height, uint32_t count, uint8_t out[DH_RECORD_BYTES]) {
    size_t end; int rc=dh_source_extent(fb,&end); if(rc)return rc;
    const struct state *s=&plane->state;
    if(!plane->id || !count || count>DH_MAX_PLANES || !width || !height || width>16384 || height>16384 || (uint64_t)width*height*4>CAPTURE_LIMIT) return INVALID_BUFFER;
    if(s->type>2 || s->alpha>65535 || (count>1 && !plane->has_z) || plane->z>INT32_MAX) return COMPOSITION;
    if((s->src_x|s->src_y|s->src_w|s->src_h)&65535) return GEOMETRY;
    uint64_t x=s->src_x>>16,y=s->src_y>>16,w=s->src_w>>16,h=s->src_h>>16;
    if(!w || !h || x+w>fb->width || y+h>fb->height) return GEOMETRY;
    if((int64_t)s->x<INT32_MIN || (int64_t)s->x>INT32_MAX || (int64_t)s->y<INT32_MIN || (int64_t)s->y>INT32_MAX) return GEOMETRY;
    if(s->rotation & ~(uint64_t)(DRM_MODE_ROTATE_MASK|DRM_MODE_REFLECT_MASK)) return GEOMETRY;
    unsigned rotation;
    switch(s->rotation & DRM_MODE_ROTATE_MASK) {
        case DRM_MODE_ROTATE_0:rotation=0;break;
        case DRM_MODE_ROTATE_90:rotation=3;break; /* DRM is counterclockwise. */
        case DRM_MODE_ROTATE_180:rotation=2;break;
        case DRM_MODE_ROTATE_270:rotation=1;break;
        default:return GEOMETRY;
    }
    uint64_t dw=(rotation&1)?h:w,dh=(rotation&1)?w:h;
    if(s->w!=dw || s->h!=dh) return GEOMETRY;
    unsigned blend=0;
    if(fb->pixel_format==DRM_FORMAT_ARGB8888) {
        if(!plane->has_blend || plane->blend>2) return COMPOSITION;
        blend=plane->blend==0?2:plane->blend==1?1:0;
    }
    if(s->type==1 && (s->alpha!=65535 || blend!=0 || s->x || s->y || dw!=width || dh!=height))return COMPOSITION;
    memset(out,0,DH_RECORD_BYTES);
    dh_u32(out,plane->id);dh_u32(out+4,plane->has_z?(uint32_t)plane->z:0);
    out[8]=s->type==1?0:s->type==0?1:2;out[9]=fb->pixel_format==DRM_FORMAT_ARGB8888?1:2;
    out[10]=(uint8_t)blend;out[11]=(uint8_t)rotation;
    dh_u16(out+12,(s->rotation&DRM_MODE_REFLECT_X?1:0)|(s->rotation&DRM_MODE_REFLECT_Y?2:0));dh_u16(out+14,(uint16_t)s->alpha);
    dh_u32(out+16,fb->width);dh_u32(out+20,fb->height);dh_u32(out+24,fb->width*4);
    dh_u32(out+28,(uint32_t)x);dh_u32(out+32,(uint32_t)y);dh_u32(out+36,(uint32_t)w);dh_u32(out+40,(uint32_t)h);
    dh_u32(out+44,(uint32_t)s->x);dh_u32(out+48,(uint32_t)s->y);dh_u32(out+52,(uint32_t)dw);dh_u32(out+56,(uint32_t)dh);
    dh_u32(out+60,fb->width*fb->height*4);return OK;
}
static inline void dh_header(uint8_t out[DH_HEADER_BYTES], uint32_t width, uint32_t height, uint32_t count, uint32_t bytes) {
    memcpy(out,"DHP2",4);dh_u16(out+4,1);dh_u16(out+6,DH_HEADER_BYTES);
    dh_u32(out+8,width);dh_u32(out+12,height);dh_u32(out+16,count);dh_u32(out+20,bytes);
}
