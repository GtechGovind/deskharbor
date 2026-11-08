#include <jni.h>
#include <android/native_window_jni.h>
#include <cstring>
#include <climits>
#include "dh_loader.h"
#include "dh_copy.h"
namespace {
const DhApi& api(){static const DhApi a=dh_load_api();return a;}
struct Lease {DhFrame frame{};~Lease(){if(frame.lease)api().release(frame.lease);}};
struct Window {ANativeWindow* value;~Window(){if(value)ANativeWindow_release(value);}};
}
extern "C" JNIEXPORT jboolean JNICALL Java_dev_deskharbor_native_1surfaces_NativeSurfacesPlugin_nativeAvailable(JNIEnv*,jclass){return api().acquire&&api().release;}
extern "C" JNIEXPORT jint JNICALL Java_dev_deskharbor_native_1surfaces_NativeSurfacesPlugin_nativePresent(JNIEnv* env,jclass,jlong session,jobject producer){
 if(!api().acquire||!api().release||session<0||session>UINT32_MAX)return 2;
 Lease lease;
 const bool available=api().acquire(static_cast<uint32_t>(session),&lease.frame);
 const bool has_frame=available&&dh_frame_valid(&lease.frame)&&lease.frame.width<=INT_MAX&&lease.frame.height<=INT_MAX;
 const int presentation_status=!available?1:(has_frame?0:2);
 // A failed acquire submits a genuine transparent native clear before the
 // explicit error reaches Dart. It never acknowledges a generated video frame.
 const DhFrame empty{nullptr,0,1,1,4,nullptr};
 const auto& f=has_frame?lease.frame:empty;
 jclass cls=env->FindClass("io/flutter/view/TextureRegistry$SurfaceProducer");if(!cls){env->ExceptionClear();return 2;}
 jmethodID resize=env->GetMethodID(cls,"setSize","(II)V");
 if(!resize){env->ExceptionClear();env->DeleteLocalRef(cls);return 2;}
 jmethodID surface=env->GetMethodID(cls,"getSurface","()Landroid/view/Surface;");
 if(!surface){env->ExceptionClear();env->DeleteLocalRef(cls);return 2;}
 env->CallVoidMethod(producer,resize,static_cast<jint>(f.width),static_cast<jint>(f.height));
 if(env->ExceptionCheck()){env->ExceptionClear();env->DeleteLocalRef(cls);return 2;}
 jobject target=env->CallObjectMethod(producer,surface);env->DeleteLocalRef(cls);
 if(env->ExceptionCheck()){env->ExceptionClear();return 2;}if(!target)return 2;
 Window window{ANativeWindow_fromSurface(env,target)};env->DeleteLocalRef(target);if(!window.value)return 2;
 if(ANativeWindow_setBuffersGeometry(window.value,static_cast<int32_t>(f.width),static_cast<int32_t>(f.height),WINDOW_FORMAT_RGBA_8888)!=0)return 2;
 ANativeWindow_Buffer buffer{};if(ANativeWindow_lock(window.value,&buffer,nullptr)!=0)return 2;
 bool valid=buffer.bits&&buffer.width==static_cast<int32_t>(f.width)&&buffer.height==static_cast<int32_t>(f.height)&&buffer.stride>=buffer.width&&buffer.format==WINDOW_FORMAT_RGBA_8888&&static_cast<size_t>(buffer.stride)<=SIZE_MAX/4;
 if(valid){
  // NativeWindow is top-left RGBA; honor destination row padding. Android owns
  // the copy after unlockAndPost. No Rust lease survives this synchronous call.
  const size_t stride=static_cast<size_t>(buffer.stride)*4;
  valid=static_cast<size_t>(buffer.height)<=SIZE_MAX/stride;
  if(valid&&has_frame)valid=dh_copy_rgba(&f,static_cast<uint8_t*>(buffer.bits),stride,stride*buffer.height,false);
  else if(valid)std::memset(buffer.bits,0,stride*buffer.height);
 }
 const int posted=ANativeWindow_unlockAndPost(window.value);return valid&&posted==0?presentation_status:2;
}
