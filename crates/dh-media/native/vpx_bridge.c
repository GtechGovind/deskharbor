#include <stdint.h>
#include <stddef.h>
#include <stdlib.h>
#include <string.h>
#include <limits.h>
#include <vpx/vpx_encoder.h>
#include <vpx/vpx_decoder.h>
#include <vpx/vp8cx.h>
#include <vpx/vp8dx.h>

typedef struct { vpx_codec_ctx_t codec; vpx_image_t image; int64_t pts; } dh_encoder;
typedef struct { vpx_codec_ctx_t codec; } dh_decoder;
typedef struct {
    const uint8_t *y, *u, *v;
    int32_t y_stride, u_stride, v_stride;
    uint32_t width, height;
} dh_yuv_frame;

void *dh_vpx_encoder_new(uint32_t width, uint32_t height, uint32_t kbps) {
    vpx_codec_enc_cfg_t config;
    if (!width || !height || width>4096 || height>4096 || kbps<64 || kbps>100000) return NULL;
    dh_encoder *encoder=(dh_encoder *)calloc(1,sizeof(*encoder));
    if (!encoder) return NULL;
    if (vpx_codec_enc_config_default(vpx_codec_vp8_cx(),&config,0)!=VPX_CODEC_OK) { free(encoder);return NULL; }
    config.g_w=width;config.g_h=height;config.g_timebase.num=1;config.g_timebase.den=30;
    config.g_threads=2;config.g_lag_in_frames=0;config.rc_end_usage=VPX_CBR;
    config.rc_target_bitrate=kbps;config.rc_min_quantizer=4;config.rc_max_quantizer=56;
    config.kf_mode=VPX_KF_AUTO;config.kf_min_dist=0;config.kf_max_dist=120;
    if (vpx_codec_enc_init(&encoder->codec,vpx_codec_vp8_cx(),&config,0)!=VPX_CODEC_OK) {free(encoder);return NULL;}
    if (!vpx_img_alloc(&encoder->image,VPX_IMG_FMT_I420,width,height,1)) {vpx_codec_destroy(&encoder->codec);free(encoder);return NULL;}
    if (vpx_codec_control(&encoder->codec,VP8E_SET_CPUUSED,8)!=VPX_CODEC_OK) {
        vpx_img_free(&encoder->image);vpx_codec_destroy(&encoder->codec);free(encoder);return NULL;
    }
    return encoder;
}
void dh_vpx_encoder_free(void *handle) {
    if (!handle) return;
    dh_encoder *encoder=(dh_encoder *)handle;
    vpx_img_free(&encoder->image);vpx_codec_destroy(&encoder->codec);free(encoder);
}
int dh_vpx_encode(void *handle,const uint8_t *i420,size_t length,int force_keyframe,const uint8_t **data,size_t *size,int *keyframe) {
    if (!handle||!i420||!data||!size||!keyframe) return 0;
    dh_encoder *encoder=(dh_encoder *)handle;
    size_t width=encoder->image.d_w,height=encoder->image.d_h,cw=(width+1)/2,ch=(height+1)/2;
    if (length!=width*height+2*cw*ch||encoder->pts==INT64_MAX) return 0;
    const uint8_t *planes[3]={i420,i420+width*height,i420+width*height+cw*ch};
    for (int plane=0;plane<3;plane++) {
        size_t rows=plane?ch:height,cols=plane?cw:width;
        for(size_t row=0;row<rows;row++) memcpy(encoder->image.planes[plane]+row*(size_t)encoder->image.stride[plane],planes[plane]+row*cols,cols);
    }
    if (vpx_codec_encode(&encoder->codec,&encoder->image,encoder->pts++,1,force_keyframe?VPX_EFLAG_FORCE_KF:0,VPX_DL_REALTIME)!=VPX_CODEC_OK) return 0;
    vpx_codec_iter_t iterator=NULL;const vpx_codec_cx_pkt_t *packet;
    while((packet=vpx_codec_get_cx_data(&encoder->codec,&iterator))!=NULL) {
        if(packet->kind==VPX_CODEC_CX_FRAME_PKT) {
            if(packet->data.frame.sz==0||packet->data.frame.sz>2*1024*1024) return 0;
            *data=(const uint8_t *)packet->data.frame.buf;*size=packet->data.frame.sz;
            *keyframe=(packet->data.frame.flags&VPX_FRAME_IS_KEY)!=0;return 1;
        }
    }
    return 0;
}
void *dh_vpx_decoder_new(void) {
    dh_decoder *decoder=(dh_decoder *)calloc(1,sizeof(*decoder));
    if (!decoder) return NULL;
    vpx_codec_dec_cfg_t config;memset(&config,0,sizeof(config));config.threads=2;
    if(vpx_codec_dec_init(&decoder->codec,vpx_codec_vp8_dx(),&config,0)!=VPX_CODEC_OK) {free(decoder);return NULL;}
    return decoder;
}
void dh_vpx_decoder_free(void *handle) {
    if(!handle) return;dh_decoder *decoder=(dh_decoder *)handle;
    vpx_codec_destroy(&decoder->codec);free(decoder);
}
int dh_vpx_decode(void *handle,const uint8_t *data,size_t length,dh_yuv_frame *frame) {
    if(!handle||!data||!frame||length<3||length>2*1024*1024) return 0;
    if(!(data[0]&1)) {
        if(length<10||data[3]!=0x9d||data[4]!=1||data[5]!=0x2a) return 0;
        unsigned width=((unsigned)data[6]|(unsigned)data[7]<<8),height=((unsigned)data[8]|(unsigned)data[9]<<8);
        if(!width||!height||width>4096||height>4096) return 0;
    }
    dh_decoder *decoder=(dh_decoder *)handle;
    if(vpx_codec_decode(&decoder->codec,data,(unsigned int)length,NULL,0)!=VPX_CODEC_OK) return 0;
    vpx_codec_iter_t iterator=NULL;vpx_image_t *image=vpx_codec_get_frame(&decoder->codec,&iterator);
    if(!image||image->fmt!=VPX_IMG_FMT_I420||!image->d_w||!image->d_h||image->d_w>4096||image->d_h>4096) return 0;
    if(!image->planes[0]||!image->planes[1]||!image->planes[2]||image->stride[0]<(int)image->d_w||image->stride[1]<(int)((image->d_w+1)/2)||image->stride[2]<(int)((image->d_w+1)/2)) return 0;
    frame->y=image->planes[0];frame->u=image->planes[1];frame->v=image->planes[2];
    frame->y_stride=image->stride[0];frame->u_stride=image->stride[1];frame->v_stride=image->stride[2];
    frame->width=image->d_w;frame->height=image->d_h;return 1;
}
