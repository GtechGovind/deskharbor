/* Synthetic test-only producer. No device or operating-system capture access. */
#include "plane_render.h"
#include <stdio.h>
static int write_visible(const void *bytes,size_t length){return fwrite(bytes,1,length,stdout)==length?OK:UNAVAILABLE;}
int main(int argc,char **argv) {
    if(argc>2 || (argc==2 && strcmp(argv[1],"--visible")))return BAD_REQUEST;
    struct dh_plane primary={.id=1,.z=0,.has_z=1,.has_blend=1,.blend=2,
        .state={.crtc=1,.fb=1,.src_w=131072,.src_h=65536,.w=2,.h=1,.rotation=1,.type=1,.alpha=65535}};
    struct dh_plane cursor={.id=2,.z=1,.has_z=1,.has_blend=1,.blend=1,
        .state={.crtc=1,.fb=2,.src_w=65536,.src_h=65536,.w=1,.h=1,.rotation=1,.type=2,.alpha=65535}};
    struct drm_mode_fb_cmd2 main_fb={.width=2,.height=1,.pixel_format=DRM_FORMAT_XRGB8888,.flags=DRM_MODE_FB_MODIFIERS,.pitches={8}};
    struct drm_mode_fb_cmd2 cursor_fb={.width=1,.height=1,.pixel_format=DRM_FORMAT_ARGB8888,.flags=DRM_MODE_FB_MODIFIERS,.pitches={4}};
    uint8_t header[24],base[64],top[64];
    uint8_t blue[]={255,0,0,0,255,0,0,0},red[]={0,0,255,128};
    if(dh_record(&primary,&main_fb,2,1,2,base) || dh_record(&cursor,&cursor_fb,2,1,2,top))return 1;
    if(argc==2) {
        struct dh_image images[2]={{.pixels=blue,.bytes=sizeof blue},{.pixels=red,.bytes=sizeof red}};
        memcpy(images[0].record,base,64);memcpy(images[1].record,top,64);
        return dh_emit_visible(write_visible,2,1,images,2);
    }
    dh_header(header,2,1,2,12);
    if(fwrite(header,1,sizeof header,stdout)!=sizeof header || fwrite(base,1,sizeof base,stdout)!=sizeof base ||
        fwrite(blue,1,sizeof blue,stdout)!=sizeof blue || fwrite(top,1,sizeof top,stdout)!=sizeof top ||
        fwrite(red,1,sizeof red,stdout)!=sizeof red)return 1;
    return 0;
}
