#pragma once
#include "plane_packet.h"
#include <stdlib.h>
struct dh_image { uint8_t record[DH_RECORD_BYTES]; uint8_t *pixels; uint32_t bytes; };
typedef int (*dh_writer)(const void *,size_t);
static inline uint16_t dh_get16(const uint8_t *p) {return (uint16_t)p[0]|((uint16_t)p[1]<<8);}
static inline uint32_t dh_get32(const uint8_t *p) {return (uint32_t)p[0]|((uint32_t)p[1]<<8)|((uint32_t)p[2]<<16)|((uint32_t)p[3]<<24);}
static inline int64_t dh_signed32(const uint8_t *p) {uint32_t v=dh_get32(p);return v>INT32_MAX?(int64_t)v-4294967296LL:(int64_t)v;}

static inline int dh_image_valid(const struct dh_image *image) {
    const uint8_t *r=image->record;
    uint32_t w=dh_get32(r+16),h=dh_get32(r+20),stride=dh_get32(r+24);
    if(!image->pixels || !w || !h || w>16384 || h>16384 || stride<(uint64_t)w*4 ||
        (uint64_t)stride*h!=image->bytes || image->bytes>CAPTURE_LIMIT || dh_get32(r+60)!=image->bytes)return INVALID_BUFFER;
    if(!dh_get32(r) || r[8]>2 || r[9]>2 || r[10]>2 || r[11]>3 || (dh_get16(r+12)&~3))return COMPOSITION;
    uint32_t x=dh_get32(r+28),y=dh_get32(r+32),cw=dh_get32(r+36),ch=dh_get32(r+40);
    if(!cw || !ch || (uint64_t)x+cw>w || (uint64_t)y+ch>h)return GEOMETRY;
    uint32_t dw=(r[11]&1)?ch:cw,dh=(r[11]&1)?cw:ch;
    return dh_get32(r+52)==dw && dh_get32(r+56)==dh?OK:GEOMETRY;
}

static inline int dh_render_plane(uint8_t *output,uint32_t width,uint32_t height,const struct dh_image *image) {
    const uint8_t *r=image->record;
    int64_t dx=dh_signed32(r+44),dy=dh_signed32(r+48);
    uint32_t dw=dh_get32(r+52),dh=dh_get32(r+56),cw=dh_get32(r+36),ch=dh_get32(r+40),stride=dh_get32(r+24);
    int64_t left=dx>0?dx:0,top=dy>0?dy:0,right=dx+dw,bottom=dy+dh;
    if(right>width)right=width;
    if(bottom>height)bottom=height;
    for(int64_t y=top;y<bottom;y++)for(int64_t x=left;x<right;x++) {
        uint32_t ox=(uint32_t)(x-dx),oy=(uint32_t)(y-dy),sx=0,sy=0;
        switch(r[11]) {
            case 0:sx=ox;sy=oy;break;
            case 1:sx=oy;sy=ch-1-ox;break;
            case 2:sx=cw-1-ox;sy=ch-1-oy;break;
            case 3:sx=cw-1-oy;sy=ox;break;
        }
        if(dh_get16(r+12)&1)sx=cw-1-sx;
        if(dh_get16(r+12)&2)sy=ch-1-sy;
        sx+=dh_get32(r+28);sy+=dh_get32(r+32);
        const uint8_t *pixel=image->pixels+(size_t)sy*stride+(size_t)sx*4;
        uint8_t rgba[]={pixel[r[9]?2:0],pixel[1],pixel[r[9]?0:2],r[9]==2?255:pixel[3]};
        if(r[10]==2 && (rgba[0]>rgba[3] || rgba[1]>rgba[3] || rgba[2]>rgba[3]))return COMPOSITION;
        uint64_t alpha=r[10]==0?255:rgba[3],opacity=dh_get16(r+14),denominator=255u*65535u;
        size_t at=((size_t)y*width+(size_t)x)*4;
        for(unsigned c=0;c<3;c++) {
            uint64_t source=(uint64_t)rgba[c]*(r[10]==2?255:alpha)*opacity;
            uint64_t behind=(uint64_t)output[at+c]*(denominator-alpha*opacity);
            output[at+c]=(uint8_t)((source+behind+denominator/2)/denominator);
        }
        output[at+3]=255;
    }
    return OK;
}
/* Internal trusted-side API: each pixels pointer owns at least bytes readable bytes.
 * Never accepts arbitrary process pointers or a remote caller's descriptors. */
static inline int dh_emit_visible(dh_writer writer,uint32_t width,uint32_t height,const struct dh_image *images,uint32_t count) {
    if(!writer || !images || !width || !height || width>16384 || height>16384 ||
        (uint64_t)width*height*4>CAPTURE_LIMIT || !count || count>DH_MAX_PLANES)return INVALID_BUFFER;
    uint32_t order[DH_MAX_PLANES];uint64_t total=0;unsigned primary=0;
    for(uint32_t i=0;i<count;i++) {
        int rc=dh_image_valid(&images[i]);if(rc)return rc;
        total+=images[i].bytes;if(total>CAPTURE_LIMIT)return INVALID_BUFFER;
        const uint8_t *r=images[i].record;
        for(uint32_t j=0;j<i;j++)if(dh_get32(images[j].record)==dh_get32(r) || dh_get32(images[j].record+4)==dh_get32(r+4))return COMPOSITION;
        primary+=r[8]==0;
        uint32_t at=i;
        while(at && dh_signed32(images[order[at-1]].record+4)>dh_signed32(r+4)) {order[at]=order[at-1];at--;}
        order[at]=i;
    }
    const uint8_t *base=images[order[0]].record;
    if(primary!=1 || base[8]!=0 || base[10]!=0 || dh_get16(base+14)!=65535 || dh_signed32(base+44) || dh_signed32(base+48) ||
        dh_get32(base+52)!=width || dh_get32(base+56)!=height)return COMPOSITION;
    size_t bytes=(size_t)width*height*4;
    uint8_t *visible=calloc(1,bytes);if(!visible)return INVALID_BUFFER;
    int rc=OK;
    for(uint32_t i=0;i<count;i++)if((rc=dh_render_plane(visible,width,height,&images[order[i]]))) {free(visible);return rc;}
    uint8_t header[12];memcpy(header,"DHF1",4);dh_u32(header+4,width);dh_u32(header+8,height);
    rc=writer(header,sizeof header);
    if(!rc)rc=writer(visible,bytes);
    free(visible);return rc;
}
