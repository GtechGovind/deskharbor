#include "plane_render.h"
#include <stdio.h>
static uint8_t published[1024];static size_t length;
static int collect(const void *bytes,size_t size) {
    if(size>sizeof published-length)return INVALID_BUFFER;
    memcpy(published+length,bytes,size);length+=size;return OK;
}
static int failures;
#define CHECK(expr) do{if(!(expr)){fprintf(stderr,"render failed line %d: %s\n",__LINE__,#expr);failures++;}}while(0)
int main(void) {
    struct dh_plane p={.id=1,.z=0,.has_z=1,.has_blend=1,.blend=2,
        .state={.crtc=1,.fb=1,.src_x=65536,.src_w=65536,.src_h=65536,.w=1,.h=1,.rotation=1,.type=1,.alpha=65535}};
    struct drm_mode_fb_cmd2 fb={.width=3,.height=1,.pixel_format=DRM_FORMAT_XRGB8888,.flags=DRM_MODE_FB_MODIFIERS,.pitches={12}};
    uint8_t private_source[]={171,202,233,0,0,255,0,0,171,202,233,0};
    struct dh_image image={.pixels=private_source,.bytes=sizeof private_source};
    CHECK(dh_record(&p,&fb,1,1,1,image.record)==OK);
    CHECK(dh_emit_visible(collect,1,1,&image,1)==OK);
    const uint8_t green[]={0,255,0,255};
    CHECK(length==16 && !memcmp(published,"DHF1",4) && !memcmp(published+12,green,4));
    /* An opaque overlay must prevent hidden primary pixels crossing the pipe. */
    length=0;p.state.src_x=0;fb.width=1;fb.pitches[0]=4;
    image.bytes=4;CHECK(dh_record(&p,&fb,1,1,2,image.record)==OK);
    struct dh_plane top=p;top.id=2;top.z=1;top.state.type=0;
    uint8_t visible_green[]={0,255,0,0};
    struct dh_image images[2]={image,{.pixels=visible_green,.bytes=4}};
    CHECK(dh_record(&top,&fb,1,1,2,images[1].record)==OK);
    CHECK(dh_emit_visible(collect,1,1,images,2)==OK);
    CHECK(length==16 && !memcmp(published+12,green,4));
    CHECK(dh_emit_visible(collect,0,1,images,2)==INVALID_BUFFER);
    /* Native trusted-side blending must agree with the independent Rust fixture. */
    length=0;uint8_t blue[]={255,0,0,0},premult[]={0,0,128,128};
    images[0].pixels=blue;top.state.type=2;top.state.alpha=32768;top.blend=0;
    fb.pixel_format=DRM_FORMAT_ARGB8888;images[1].pixels=premult;
    CHECK(dh_record(&top,&fb,1,1,2,images[1].record)==OK);
    CHECK(dh_emit_visible(collect,1,1,images,2)==OK);
    const uint8_t half[]={64,0,191,255};CHECK(length==16 && !memcmp(published+12,half,4));
    /* Asymmetric six-pixel fixture; DRM270 is clockwise90 in the renderer. */
    length=0;uint8_t six[]={0,0,1,0,0,0,2,0,0,0,3,0,0,0,4,0,0,0,5,0,0,0,6,0};
    p.state.src_w=131072;p.state.src_h=196608;p.state.w=3;p.state.h=2;p.state.rotation=DRM_MODE_ROTATE_270;
    fb.width=2;fb.height=3;fb.pitches[0]=8;fb.pixel_format=DRM_FORMAT_XRGB8888;
    image.pixels=six;image.bytes=sizeof six;
    CHECK(dh_record(&p,&fb,3,2,1,image.record)==OK && dh_emit_visible(collect,3,2,&image,1)==OK);
    const uint8_t turns[]={5,3,1,6,4,2};for(unsigned i=0;i<6;i++)CHECK(published[12+i*4]==turns[i]);
    length=0;p.state.rotation|=DRM_MODE_REFLECT_X;
    CHECK(dh_record(&p,&fb,3,2,1,image.record)==OK && dh_emit_visible(collect,3,2,&image,1)==OK);
    const uint8_t reflected[]={6,4,2,5,3,1};for(unsigned i=0;i<6;i++)CHECK(published[12+i*4]==reflected[i]);
    return failures!=0;
}
