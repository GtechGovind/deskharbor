#include "plane_packet.h"
#include <stdio.h>

static int failures;
#define CHECK(expr) do { if (!(expr)) { fprintf(stderr,"plane failed line %d: %s\n",__LINE__,#expr); failures++; } } while (0)
int main(void) {
    CHECK(dh_plane_property("AMD_PLANE_CTM",1)==COMPOSITION);
    CHECK(dh_plane_property("unknown_effect",0)==COMPOSITION);
    CHECK(dh_plane_property("HOTSPOT_X",1)==GEOMETRY);
    CHECK(dh_plane_property("IN_FORMATS",42)==OK);
    CHECK(dh_plane_property("COLOR_PIPELINE",4)==COMPOSITION);
    struct dh_plane p = {.id=7,.z=0,.has_z=1,.has_blend=1,.blend=2,
        .state={.crtc=9,.fb=3,.width=2,.height=1,.planes=1,.src_w=131072,.src_h=65536,.w=2,.h=1,.rotation=1,.type=1,.alpha=65535}};
    struct drm_mode_fb_cmd2 fb = {.width=2,.height=1,.pixel_format=DRM_FORMAT_XRGB8888,.flags=DRM_MODE_FB_MODIFIERS,.pitches={8}};
    size_t end=0; uint8_t record[DH_RECORD_BYTES]={0}, header[DH_HEADER_BYTES]={0};
    CHECK(dh_source_extent(&fb,&end)==OK && end==8);
    CHECK(dh_record(&p,&fb,2,1,1,record)==OK);
    CHECK(record[0]==7 && record[8]==0 && record[9]==2 && record[10]==0 && record[11]==0);
    CHECK(record[14]==255 && record[15]==255 && record[60]==8);
    dh_header(header,2,1,2,12);
    const uint8_t expected[]={68,72,80,50,1,0,24,0,2,0,0,0,1,0,0,0,2,0,0,0,12,0,0,0};
    CHECK(!memcmp(header,expected,sizeof expected));
    p.state.rotation=DRM_MODE_ROTATE_90;p.state.w=1;p.state.h=2;
    CHECK(dh_record(&p,&fb,1,2,1,record)==OK && record[11]==3); /* DRM CCW -> packet CW */
    p.state.rotation=DRM_MODE_ROTATE_270|DRM_MODE_REFLECT_X;
    CHECK(dh_record(&p,&fb,1,2,1,record)==OK && record[11]==1 && record[12]==1);
    p.state.rotation=1;p.state.w=2;p.state.h=1;p.state.src_x=1;
    CHECK(dh_record(&p,&fb,2,1,1,record)==GEOMETRY);
    p.state.src_x=0;p.state.w=3;CHECK(dh_record(&p,&fb,2,1,1,record)==GEOMETRY);
    p.state.w=2;p.has_z=0;CHECK(dh_record(&p,&fb,2,1,2,record)==COMPOSITION);
    p.has_z=1;fb.modifier[0]=1;CHECK(dh_source_extent(&fb,&end)==MODIFIER);
    fb.modifier[0]=0;fb.pixel_format=DRM_FORMAT_ARGB8888;p.state.type=2;p.state.alpha=32768;p.blend=0;
    CHECK(dh_record(&p,&fb,2,1,2,record)==OK && record[8]==2 && record[9]==1 && record[10]==2);
    p.has_blend=0;CHECK(dh_record(&p,&fb,2,1,2,record)==COMPOSITION);
    return failures!=0;
}
