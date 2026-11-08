#pragma once
#include "plane_packet.h"
#include <errno.h>
static inline int dh_worker_identity(const uint32_t uid[3],const uint32_t gid[3],unsigned long secure,const uint32_t caps[6]) {
    if(secure)return 0;
    for(unsigned i=0;i<3;i++)if(!uid[i] || !gid[i] || uid[i]!=uid[0] || gid[i]!=gid[0])return 0;
    for(unsigned i=0;i<6;i++)if(caps[i])return 0;
    return 1;
}
static inline int dh_object_property(uint32_t type,const char *name,uint64_t value,uint32_t crtc) {
    if(type==DRM_MODE_OBJECT_PLANE)return dh_plane_property(name,value);
    if(type==DRM_MODE_OBJECT_CRTC) {
        if(!strcmp(name,"ACTIVE"))return value==1?OK:NO_SCANOUT;
        if(!strcmp(name,"MODE_ID"))return value?OK:NO_SCANOUT;
        const char *metadata[]={"GAMMA_LUT_SIZE","DEGAMMA_LUT_SIZE"};
        for(unsigned i=0;i<sizeof metadata/sizeof *metadata;i++)if(!strcmp(name,metadata[i]))return OK;
        const char *zero[]={"OUT_FENCE_PTR","CTM","GAMMA_LUT","DEGAMMA_LUT","BACKGROUND_COLOR","VRR_ENABLED","SCALING_FILTER"};
        for(unsigned i=0;i<sizeof zero/sizeof *zero;i++)if(!strcmp(name,zero[i]))return value?COMPOSITION:OK;
    } else if(type==DRM_MODE_OBJECT_CONNECTOR) {
        if(!strcmp(name,"CRTC_ID"))return value==crtc?OK:CHANGED;
        if(!strcmp(name,"DPMS") || !strcmp(name,"link-status") || !strcmp(name,"non-desktop"))return value?NO_SCANOUT:OK;
        const char *metadata[]={"EDID","PATH","vrr_capable","subconnector","audio"};
        for(unsigned i=0;i<sizeof metadata/sizeof *metadata;i++)if(!strcmp(name,metadata[i]))return OK;
        if(!strcmp(name,"max bpc"))return value==8?OK:COMPOSITION;
        if(!strcmp(name,"Broadcast RGB"))return value==1?OK:COMPOSITION; /* explicit Full */
        const char *zero[]={"TILE","Content Protection","HDR_OUTPUT_METADATA","Colorspace","panel orientation","scaling mode","underscan","underscan hborder","underscan vborder","content type"};
        for(unsigned i=0;i<sizeof zero/sizeof *zero;i++)if(!strcmp(name,zero[i]))return value?COMPOSITION:OK;
    }
    return COMPOSITION;
}
static inline int dh_mode_supported(const struct drm_mode_modeinfo *mode) {
    if(!mode->clock || !mode->hdisplay || !mode->vdisplay || mode->hdisplay>16384 || mode->vdisplay>16384 ||
        mode->hsync_start<mode->hdisplay || mode->hsync_end<mode->hsync_start || mode->htotal<mode->hsync_end ||
        mode->vsync_start<mode->vdisplay || mode->vsync_end<mode->vsync_start || mode->vtotal<mode->vsync_end || mode->vscan>1)return GEOMETRY;
    uint32_t allowed=DRM_MODE_FLAG_PHSYNC|DRM_MODE_FLAG_NHSYNC|DRM_MODE_FLAG_PVSYNC|DRM_MODE_FLAG_NVSYNC;
    if(mode->flags&~allowed)return GEOMETRY;
    if((mode->flags&(DRM_MODE_FLAG_PHSYNC|DRM_MODE_FLAG_NHSYNC))==(DRM_MODE_FLAG_PHSYNC|DRM_MODE_FLAG_NHSYNC) ||
        (mode->flags&(DRM_MODE_FLAG_PVSYNC|DRM_MODE_FLAG_NVSYNC))==(DRM_MODE_FLAG_PVSYNC|DRM_MODE_FLAG_NVSYNC))return GEOMETRY;
    return OK;
}
typedef int (*dh_set_cap)(uint64_t);
static inline int dh_visibility_caps(dh_set_cap set) {
    const uint64_t caps[]={DRM_CLIENT_CAP_ATOMIC,6,7};
    for(unsigned i=0;i<3;i++) {
        int error=set(caps[i]);
        if(!error)continue;
        /* Atomic succeeded first: optional EINVAL means unknown kernel cap;
         * EOPNOTSUPP means this driver has no hidden cursor/color interface.
         * Other failures cannot establish property/plane visibility. */
        if(i && (error==EINVAL || error==EOPNOTSUPP))continue;
        return error==EACCES || error==EPERM?PERMISSION:UNAVAILABLE;
    }
    return OK;
}
