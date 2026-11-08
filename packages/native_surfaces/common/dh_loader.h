#ifndef DESKHARBOR_DH_LOADER_H
#define DESKHARBOR_DH_LOADER_H
#include "dh_frame.h"
#ifdef _WIN32
#include <windows.h>
#else
#include <dlfcn.h>
#endif
/* Never load a distinct path/copy: these names are also used by FRB. Keep the
 * module loaded for process life because renderer callbacks may hold leases. */
typedef struct DhApi { DhAcquire acquire; DhRelease release; } DhApi;
static inline DhApi dh_load_api(void) {
 DhApi api = {0};
#ifdef _WIN32
 HMODULE module = NULL;
 GetModuleHandleExW(0, L"dh_ffi.dll", &module);
 if (!module) module = LoadLibraryExW(L"dh_ffi.dll", NULL, LOAD_LIBRARY_SEARCH_APPLICATION_DIR);
 if (module) {api.acquire=(DhAcquire)GetProcAddress(module,"dh_frame_acquire");api.release=(DhRelease)GetProcAddress(module,"dh_frame_release");}
#else
#ifdef __APPLE__
 const char *name="@rpath/libdh_ffi.dylib";
#else
 const char *name="libdh_ffi.so";
#endif
 void *module=dlopen(name,RTLD_NOW|RTLD_LOCAL);
 if (module) {api.acquire=(DhAcquire)dlsym(module,"dh_frame_acquire");api.release=(DhRelease)dlsym(module,"dh_frame_release");}
#endif
 return api;
}
#endif
