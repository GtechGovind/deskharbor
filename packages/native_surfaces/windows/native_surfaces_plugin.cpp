#include "include/native_surfaces/native_surfaces_plugin_c_api.h"
#include <flutter/method_channel.h>
#include <flutter/plugin_registrar_windows.h>
#include <flutter/standard_method_codec.h>
#include <flutter/texture_registrar.h>
#include "../common/dh_lease.h"
#include <map>
#include <memory>
#include <mutex>
#include <string>
#include <vector>
#include "../common/dh_loader.h"
namespace {
using flutter::EncodableValue;
using flutter::EncodableMap;
struct Pixels {
 DhLease lease;
 FlutterDesktopPixelBuffer buffer{};
 // Buffer descriptor stays alive until texture unregister; pixel lease can be
 // released earlier by Flutter's completion callback, never by the UI thread.
 explicit Pixels(DhFrame f,DhRelease r):lease(f.lease,r) {
  buffer.buffer=f.data;buffer.width=f.width;buffer.height=f.height;
  buffer.release_context=this;
  buffer.release_callback=[](void* p){static_cast<Pixels*>(p)->Complete();};
 }
 void Complete(){lease.Complete();}
 ~Pixels(){Complete();}
};
struct Surface {
 uint32_t session;DhApi api;
 std::mutex mutex;
 std::vector<std::unique_ptr<Pixels>> descriptors;
 std::unique_ptr<flutter::TextureVariant> texture;
 Surface(uint32_t id,DhApi a):session(id),api(a) {
  texture=std::make_unique<flutter::TextureVariant>(flutter::PixelBufferTexture([this](size_t,size_t)->const FlutterDesktopPixelBuffer* {
   std::lock_guard<std::mutex> guard(mutex);
   DhFrame f{};if(!api.acquire(session,&f))return nullptr;
   if(!dh_frame_valid(&f)){api.release(f.lease);return nullptr;}
   // A completed descriptor may be reused only after its release callback.
   for(auto& p:descriptors) if(p->lease.completed()){p->buffer.buffer=f.data;p->buffer.width=f.width;p->buffer.height=f.height;p->lease.Reset(f.lease);return &p->buffer;}
   descriptors.push_back(std::make_unique<Pixels>(f,api.release));return &descriptors.back()->buffer;
  }));
 }
};
class Plugin:public flutter::Plugin {
 flutter::TextureRegistrar* textures_;
 DhApi api_=dh_load_api();
 std::map<int64_t,std::unique_ptr<Surface>> surfaces_;
 std::unique_ptr<flutter::MethodChannel<EncodableValue>> channel_;
 void Remove(int64_t id){auto it=surfaces_.find(id);if(it==surfaces_.end())return;
  auto* surface=it->second.release();surfaces_.erase(it);
  textures_->UnregisterTexture(id,[surface](){delete surface;});
 }
 static bool Arg(const flutter::MethodCall<EncodableValue>& call,const char* key,int64_t* out){
  if(!call.arguments())return false;auto* args=std::get_if<EncodableMap>(call.arguments());if(!args)return false;
  auto it=args->find(EncodableValue(key));if(it==args->end())return false;
  if(auto n=std::get_if<int32_t>(&it->second)){*out=*n;return true;}
  if(auto n=std::get_if<int64_t>(&it->second)){*out=*n;return true;}return false;
 }
 public:
 explicit Plugin(flutter::PluginRegistrarWindows* registrar):textures_(registrar->texture_registrar()) {
  channel_=std::make_unique<flutter::MethodChannel<EncodableValue>>(registrar->messenger(),"deskharbor/native_surfaces",&flutter::StandardMethodCodec::GetInstance());
  channel_->SetMethodCallHandler([this](const auto& call,auto result){
   int64_t id;
   if(call.method_name()=="createTexture"){
    if(!Arg(call,"sessionId",&id)||id<0||id>UINT32_MAX){result->Error("bad_arguments","sessionId must be uint32");return;}
    if(!api_.acquire||!api_.release){result->Error("library_unavailable","dh_ffi.dll frame ABI unavailable");return;}
    auto surface=std::make_unique<Surface>(static_cast<uint32_t>(id),api_);
    auto texture=textures_->RegisterTexture(surface->texture.get());
    if(texture<0){result->Error("texture_unavailable","Texture registration failed");return;}
    surfaces_[texture]=std::move(surface);result->Success(EncodableValue(texture));return;
   }
   if(call.method_name()!="present"&&call.method_name()!="disposeTexture"){result->NotImplemented();return;}
   if(!Arg(call,"textureId",&id)){result->Error("bad_arguments","textureId required");return;}
   auto it=surfaces_.find(id);if(it==surfaces_.end()){result->Error("texture_unavailable","Unknown texture");return;}
   if(call.method_name()=="disposeTexture"){Remove(id);result->Success();return;}
   DhFrame f{};if(!api_.acquire(it->second->session,&f)){result->Error("frame_unavailable","No decoded frame for session");return;}
   bool valid=dh_frame_valid(&f);api_.release(f.lease);
   if(!valid){result->Error("invalid_frame","Malformed RGBA frame");return;}
   if(!textures_->MarkTextureFrameAvailable(id)){result->Error("texture_unavailable","Texture no longer registered");return;}result->Success();
  });
 }
 ~Plugin() override {channel_->SetMethodCallHandler(nullptr);while(!surfaces_.empty())Remove(surfaces_.begin()->first);}
};
}
void NativeSurfacesPluginCApiRegisterWithRegistrar(FlutterDesktopPluginRegistrarRef registrar){
 auto* r=flutter::PluginRegistrarManager::GetInstance()->GetRegistrar<flutter::PluginRegistrarWindows>(registrar);
 r->AddPlugin(std::make_unique<Plugin>(r));
}
