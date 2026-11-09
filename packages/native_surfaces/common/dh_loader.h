#ifndef DESKHARBOR_DH_LOADER_H
#define DESKHARBOR_DH_LOADER_H
#include "dh_frame.h"
#include "dh_library_path.h"
#ifdef _WIN32
#include <windows.h>
#else
#include <dlfcn.h>
#if !defined(__ANDROID__)
#include <limits.h>
#include <unistd.h>
#endif
#ifdef __APPLE__
#include <mach-o/dyld.h>
#include <stdlib.h>
#endif
#endif
/* Never load a distinct path/copy: these names are also used by FRB. Keep the
 * module loaded for process life because renderer callbacks may hold leases. */
typedef struct DhApi { DhAcquire acquire; DhRelease release; } DhApi;
static inline DhApi dh_load_api(void) {
 DhApi api = {NULL, NULL};
#ifdef _WIN32
 HMODULE module = NULL;
 GetModuleHandleExW(0, L"dh_ffi.dll", &module);
 if (!module) module = LoadLibraryExW(L"dh_ffi.dll", NULL, LOAD_LIBRARY_SEARCH_APPLICATION_DIR);
 if (module) {api.acquire=(DhAcquire)GetProcAddress(module,"dh_frame_acquire");api.release=(DhRelease)GetProcAddress(module,"dh_frame_release");}
#else
#if defined(__ANDROID__)
 const char* name="libdh_ffi.so";
#else
 char executable[PATH_MAX];char library[PATH_MAX];
#ifdef __APPLE__
 char unresolved[PATH_MAX];uint32_t size=sizeof(unresolved);
 if(_NSGetExecutablePath(unresolved,&size)!=0||!realpath(unresolved,executable))return api;
 if(!dh_bundle_library_path(executable,true,library,sizeof(library)))return api;
#else
 const ssize_t count=readlink("/proc/self/exe",executable,sizeof(executable)-1);
 if(count<=0||(size_t)count>=sizeof(executable)-1)return api;
 executable[count]='\0';
 if(!dh_bundle_library_path(executable,false,library,sizeof(library)))return api;
#endif
 const char* name=library;
#endif
 void *module=dlopen(name,RTLD_NOW|RTLD_LOCAL);
 if (module) {api.acquire=(DhAcquire)dlsym(module,"dh_frame_acquire");api.release=(DhRelease)dlsym(module,"dh_frame_release");}
#endif
 return api;
}
#endif
