#ifndef DH_LIBRARY_PATH_H
#define DH_LIBRARY_PATH_H
#include <stdbool.h>
#include <stddef.h>
#include <string.h>
/* Canonical executable path supplied by the OS; no cwd/RUNPATH dependency. */
static inline bool dh_bundle_library_path(const char* executable,bool macos,char* out,size_t capacity) {
 if(!out||capacity==0)return false;
 out[0]='\0';
 if(!executable)return false;
 const char* slash=strrchr(executable,'/');
 if(!slash)return false;
 const size_t parent=(size_t)(slash-executable)+1;
 const char* suffix=macos?"../Frameworks/libdh_ffi.dylib":"lib/libdh_ffi.so";
 const size_t tail=strlen(suffix);
 if(parent>=capacity||tail>=capacity-parent)return false;
 memcpy(out,executable,parent);memcpy(out+parent,suffix,tail+1);return true;
}
#endif
