#ifndef DH_COPY_H
#define DH_COPY_H
#include "dh_frame.h"
#include <string.h>
/* Top-left RGBA8 source. Preserve row order; only macOS requests BGRA8. */
static inline bool dh_copy_rgba(const DhFrame* f,unsigned char* out,size_t stride,size_t capacity,bool bgra){
 if(!dh_frame_valid(f)||!out||stride<f->stride||f->height>SIZE_MAX/stride||capacity<stride*f->height)return false;
 for(unsigned int y=0;y<f->height;y++){
  const unsigned char* src=f->data+(size_t)y*f->stride;unsigned char* dst=out+(size_t)y*stride;
  if(!bgra){memcpy(dst,src,f->stride);continue;}
  for(unsigned int x=0;x<f->width;x++){
   dst[4*x]=src[4*x+2];dst[4*x+1]=src[4*x+1];dst[4*x+2]=src[4*x];dst[4*x+3]=src[4*x+3];
  }
 }
 return true;
}
#endif
