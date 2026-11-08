#include "include/native_surfaces/native_surfaces_plugin.h"
#include "../common/dh_loader.h"
#include "../common/dh_copy.h"
#include <cstring>
#include <map>
#include <vector>
struct _DhTexture { FlPixelBufferTexture parent_instance;uint32_t session;DhApi api;std::vector<uint8_t>* pixels; };
struct _DhTextureClass {FlPixelBufferTextureClass parent_class;};
typedef struct _DhTexture DhTexture;
typedef struct _DhTextureClass DhTextureClass;
G_DEFINE_TYPE(DhTexture,dh_texture,fl_pixel_buffer_texture_get_type())
static gboolean copy_pixels(FlPixelBufferTexture* base,const uint8_t** buffer,uint32_t* width,uint32_t* height,GError** error){
 auto* t=reinterpret_cast<DhTexture*>(base);DhFrame f{};
 if(!t->api.acquire(t->session,&f)){g_set_error_literal(error,g_quark_from_static_string("deskharbor"),1,"No decoded frame");return FALSE;}
 if(!dh_frame_valid(&f)){t->api.release(f.lease);g_set_error_literal(error,g_quark_from_static_string("deskharbor"),2,"Malformed RGBA frame");return FALSE;}
 // This synchronous copy is the last reader of the Rust lease. Flutter's GL
 // upload reads our stable RGBA storage until its next copy_pixels callback.
 t->pixels->resize(f.length);dh_copy_rgba(&f,t->pixels->data(),f.stride,f.length,false);*width=f.width;*height=f.height;
 t->api.release(f.lease);*buffer=t->pixels->data();return TRUE;
}
static void dh_texture_finalize(GObject* object){delete reinterpret_cast<DhTexture*>(object)->pixels;G_OBJECT_CLASS(dh_texture_parent_class)->finalize(object);}
static void dh_texture_class_init(DhTextureClass* klass){FL_PIXEL_BUFFER_TEXTURE_CLASS(klass)->copy_pixels=copy_pixels;G_OBJECT_CLASS(klass)->finalize=dh_texture_finalize;}
static void dh_texture_init(DhTexture* t){t->pixels=new std::vector<uint8_t>();}
struct Plugin {FlTextureRegistrar* registrar;DhApi api;std::map<int64_t,DhTexture*> textures;};
static void remove_texture(Plugin* p,std::map<int64_t,DhTexture*>::iterator it){
 fl_texture_registrar_unregister_texture(p->registrar,FL_TEXTURE(it->second));g_object_unref(it->second);p->textures.erase(it);
}
static void destroy(gpointer data){auto* p=static_cast<Plugin*>(data);while(!p->textures.empty())remove_texture(p,p->textures.begin());g_object_unref(p->registrar);delete p;}
static bool argument(FlMethodCall* call,const char* key,int64_t* id){auto* args=fl_method_call_get_args(call);if(!args||fl_value_get_type(args)!=FL_VALUE_TYPE_MAP)return false;
 auto* val=fl_value_lookup_string(args,key);if(!val||fl_value_get_type(val)!=FL_VALUE_TYPE_INT)return false;*id=fl_value_get_int(val);return true;}
static void method(FlMethodChannel*,FlMethodCall* call,gpointer data){
 auto* p=static_cast<Plugin*>(data);const char* name=fl_method_call_get_name(call);int64_t id;
 const char* code=nullptr;const char* message=nullptr;
 if(strcmp(name,"createTexture")==0){
  if(!argument(call,"sessionId",&id)||id<0||id>UINT32_MAX){code="bad_arguments";message="sessionId must be uint32";}
  else if(!p->api.acquire||!p->api.release){code="library_unavailable";message="libdh_ffi.so frame ABI unavailable";}
  else {auto* t=reinterpret_cast<DhTexture*>(g_object_new(dh_texture_get_type(),nullptr));t->api=p->api;t->session=static_cast<uint32_t>(id);
   if(!fl_texture_registrar_register_texture(p->registrar,FL_TEXTURE(t))){g_object_unref(t);code="texture_unavailable";message="Texture registration failed";}
   else {auto tid=fl_texture_get_id(FL_TEXTURE(t));p->textures[tid]=t;g_autoptr(FlValue) value=fl_value_new_int(tid);fl_method_call_respond_success(call,value,nullptr);return;}
  }
 }else if(strcmp(name,"present")==0||strcmp(name,"disposeTexture")==0){
  if(!argument(call,"textureId",&id)){code="bad_arguments";message="textureId required";}
  else {auto it=p->textures.find(id);if(it==p->textures.end()){code="texture_unavailable";message="Unknown texture";}
   else if(strcmp(name,"disposeTexture")==0){remove_texture(p,it);fl_method_call_respond_success(call,nullptr,nullptr);return;}
   else {DhFrame f{};if(!p->api.acquire(it->second->session,&f)){code="frame_unavailable";message="No decoded frame";}
    else {bool valid=dh_frame_valid(&f);p->api.release(f.lease);if(!valid){code="invalid_frame";message="Malformed RGBA frame";}
     else if(!fl_texture_registrar_mark_texture_frame_available(p->registrar,FL_TEXTURE(it->second))){code="texture_unavailable";message="Texture is unavailable";}
     else {fl_method_call_respond_success(call,nullptr,nullptr);return;}}
   }
  }
 }else {fl_method_call_respond_not_implemented(call,nullptr);return;}
 fl_method_call_respond_error(call,code,message,nullptr,nullptr);
}
void native_surfaces_plugin_register_with_registrar(FlPluginRegistrar* registrar){
 auto* p=new Plugin{FL_TEXTURE_REGISTRAR(g_object_ref(fl_plugin_registrar_get_texture_registrar(registrar))),dh_load_api(),{}};
 g_autoptr(FlStandardMethodCodec) codec=fl_standard_method_codec_new();
 g_autoptr(FlMethodChannel) channel=fl_method_channel_new(fl_plugin_registrar_get_messenger(registrar),"deskharbor/native_surfaces",FL_METHOD_CODEC(codec));
 fl_method_channel_set_method_call_handler(channel,method,p,destroy);
}
