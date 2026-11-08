#include "visibility.h"
#include <stdio.h>
static int failures;
#define CHECK(expr)do{if(!(expr)){fprintf(stderr,"visibility failed line %d: %s\n",__LINE__,#expr);failures++;}}while(0)
static unsigned calls;static uint64_t caps[3];static int error6,error7;
static int set(uint64_t cap){if(calls<3)caps[calls]=cap;calls++;return cap==6?error6:cap==7?error7:0;}
int main(void) {
    uint32_t uid[]={1000,1000,1000},gid[]={1000,1000,1000},privilege[6]={0};
    CHECK(dh_worker_identity(uid,gid,0,privilege));
    privilege[0]=1u<<21;CHECK(!dh_worker_identity(uid,gid,0,privilege));
    privilege[0]=0;uid[2]=0;CHECK(!dh_worker_identity(uid,gid,0,privilege));uid[2]=1000;
    CHECK(!dh_worker_identity(uid,gid,1,privilege));
    CHECK(dh_object_property(DRM_MODE_OBJECT_CONNECTOR,"DPMS",3,9)==NO_SCANOUT);
    CHECK(dh_object_property(DRM_MODE_OBJECT_CRTC,"ACTIVE",0,9)==NO_SCANOUT);
    CHECK(dh_object_property(DRM_MODE_OBJECT_CONNECTOR,"CRTC_ID",10,9)==CHANGED);
    for(unsigned i=0;i<3;i++)for(unsigned v=0;v<2;v++) {
        uint32_t types[]={DRM_MODE_OBJECT_CONNECTOR,DRM_MODE_OBJECT_CRTC,DRM_MODE_OBJECT_PLANE};
        CHECK(dh_object_property(types[i],"unknown_effect",v,9)==COMPOSITION);
    }
    struct drm_mode_modeinfo mode={.clock=60000,.hdisplay=2,.hsync_start=3,.hsync_end=4,.htotal=5,.vdisplay=2,.vsync_start=3,.vsync_end=4,.vtotal=5};
    CHECK(dh_mode_supported(&mode)==OK);
    mode.flags=DRM_MODE_FLAG_INTERLACE;CHECK(dh_mode_supported(&mode)==GEOMETRY);
    mode.flags=DRM_MODE_FLAG_3D_TOP_AND_BOTTOM;CHECK(dh_mode_supported(&mode)==GEOMETRY);
    mode.flags=0;mode.vscan=2;CHECK(dh_mode_supported(&mode)==GEOMETRY);
    error6=EOPNOTSUPP;error7=EINVAL;CHECK(dh_visibility_caps(set)==OK && calls==3 && caps[0]==DRM_CLIENT_CAP_ATOMIC && caps[1]==6 && caps[2]==7);
    calls=0;error6=EPERM;CHECK(dh_visibility_caps(set)==PERMISSION && calls==2);
    calls=0;error6=0;error7=EIO;CHECK(dh_visibility_caps(set)==UNAVAILABLE && calls==3);
    return failures!=0;
}
