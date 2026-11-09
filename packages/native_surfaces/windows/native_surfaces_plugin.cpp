#include "include/native_surfaces/native_surfaces_plugin_c_api.h"
#include <flutter/method_channel.h>
#include <flutter/plugin_registrar_windows.h>
#include <flutter/standard_method_codec.h>
#include <flutter/texture_registrar.h>
#include "../common/dh_presentation.h"
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
 std::shared_ptr<DhSnapshot> snapshot;
 FlutterDesktopPixelBuffer buffer{};
 explicit Pixels(std::shared_ptr<DhSnapshot> value) {
  Set(std::move(value));
  buffer.release_context=this;
  buffer.release_callback=[](void* context){
   static_cast<Pixels*>(context)->Complete();
  };
 }
 void Set(std::shared_ptr<DhSnapshot> value) {
  buffer.buffer=value->data();
  buffer.width=value->width();
  buffer.height=value->height();
  std::atomic_store(&snapshot,std::move(value));
 }
 bool Completed() const {return !std::atomic_load(&snapshot);}
 void Complete(){std::atomic_store(&snapshot,std::shared_ptr<DhSnapshot>());}
};
struct Surface {
 uint32_t session;
 DhPresentation presentation;
 std::mutex mutex;
 std::vector<std::unique_ptr<Pixels>> descriptors;
 std::unique_ptr<flutter::TextureVariant> texture;
 explicit Surface(uint32_t id):session(id) {
  texture=std::make_unique<flutter::TextureVariant>(flutter::PixelBufferTexture(
   [this](size_t,size_t)->const FlutterDesktopPixelBuffer* {
    std::lock_guard<std::mutex> guard(mutex);
    // Present acquired exactly this snapshot. No second engine lookup can fail
    // or substitute an old frame between the UI acknowledgement and upload.
    auto snapshot=presentation.ForUpload();
    for(auto& descriptor:descriptors) {
     if(descriptor->Completed()) {
      descriptor->Set(std::move(snapshot));
      return &descriptor->buffer;
     }
    }
    descriptors.push_back(std::make_unique<Pixels>(std::move(snapshot)));
    return &descriptors.back()->buffer;
   }));
 }
};
class Plugin:public flutter::Plugin {
 flutter::TextureRegistrar* textures_;
 DhApi api_{};
 std::map<int64_t,std::unique_ptr<Surface>> surfaces_;
 std::unique_ptr<flutter::MethodChannel<EncodableValue>> channel_;
 void Remove(int64_t id){auto it=surfaces_.find(id);if(it==surfaces_.end())return;
  it->second->presentation.Clear();
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
    if(!api_.acquire||!api_.release)api_=dh_load_api();
    if(!api_.acquire||!api_.release){result->Error("library_unavailable","dh_ffi.dll frame ABI unavailable");return;}
    auto surface=std::make_unique<Surface>(static_cast<uint32_t>(id));
    auto texture=textures_->RegisterTexture(surface->texture.get());
    if(texture<0){result->Error("texture_unavailable","Texture registration failed");return;}
    surfaces_[texture]=std::move(surface);result->Success(EncodableValue(texture));return;
   }
   if(call.method_name()!="present"&&call.method_name()!="disposeTexture"){result->NotImplemented();return;}
   if(!Arg(call,"textureId",&id)){result->Error("bad_arguments","textureId required");return;}
   auto it=surfaces_.find(id);if(it==surfaces_.end()){result->Error("texture_unavailable","Unknown texture");return;}
   if(call.method_name()=="disposeTexture"){Remove(id);result->Success();return;}
   const auto state=it->second->presentation.Present(api_.acquire,api_.release,it->second->session);
   // Empty is an actual transparent native presentation. Mark it even when
   // returning an error, so previously uploaded content is invalidated.
   if(!textures_->MarkTextureFrameAvailable(id)) {
    result->Error("texture_unavailable","Texture no longer registered");return;
   }
   const auto error=DhPresentationError(state);
   if(error.code[0]){result->Error(error.code,error.message);return;}
   result->Success();
  });
 }
 ~Plugin() override {channel_->SetMethodCallHandler(nullptr);while(!surfaces_.empty())Remove(surfaces_.begin()->first);}
};
}
void NativeSurfacesPluginCApiRegisterWithRegistrar(FlutterDesktopPluginRegistrarRef registrar){
 auto* r=flutter::PluginRegistrarManager::GetInstance()->GetRegistrar<flutter::PluginRegistrarWindows>(registrar);
 r->AddPlugin(std::make_unique<Plugin>(r));
}
