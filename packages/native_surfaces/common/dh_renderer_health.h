#ifndef DH_RENDERER_HEALTH_H
#define DH_RENDERER_HEALTH_H
#include <atomic>
enum class DhRenderFailure {none,egl_setup,copy_failed};
struct DhRendererError {const char* code;const char* message;};
class DhRendererHealth {
 std::atomic<DhRenderFailure> failure_{DhRenderFailure::none};
public:
 void RecordFailure(DhRenderFailure failure) {
  if(failure==DhRenderFailure::none)return;
  auto expected=DhRenderFailure::none;
  failure_.compare_exchange_strong(expected,failure);
 }
 bool failed() const{return failure_.load()!=DhRenderFailure::none;}
 DhRendererError PublicError() const {
  switch(failure_.load()) {
   case DhRenderFailure::egl_setup:return {"renderer_initialization_failed","EGL GLES renderer setup failed; restart the application or use a supported renderer"};
   case DhRenderFailure::copy_failed:return {"renderer_copy_failed","Native raster copy failed; lower the display resolution and restart the application"};
   default:return {"",""};
  }
 }
};
template<class Setup,class Copy> bool DhPrepareRaster(DhRendererHealth& health,Setup setup,Copy copy) {
 if(!setup()){health.RecordFailure(DhRenderFailure::egl_setup);return false;}
 if(!copy()){health.RecordFailure(DhRenderFailure::copy_failed);return false;}
 return true;
}
#endif
