#ifndef DESKHARBOR_DH_FRAME_H
#define DESKHARBOR_DH_FRAME_H
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#ifdef __cplusplus
extern "C" {
#endif
typedef struct DhFrame {
 const unsigned char *data;
 size_t length;
 unsigned int width, height, stride;
 void *lease;
} DhFrame;
typedef bool (*DhAcquire)(unsigned int, DhFrame *);
typedef void (*DhRelease)(void *);
/* RGBA8, top-left origin, no padding. A successful acquire transfers one
 * opaque lease; release once only after the renderer/copy stops reading. */
bool dh_frame_acquire(unsigned int session_id, DhFrame *out);
void dh_frame_release(void *lease);
static inline bool dh_frame_valid(const DhFrame *f) {
 return f && f->data && f->lease && f->width && f->height &&
   f->width <= UINT32_MAX / 4 && f->stride == f->width * 4 &&
   f->height <= SIZE_MAX / f->stride && f->length == (size_t)f->stride*f->height;
}
#ifdef __cplusplus
}
#endif
#endif
