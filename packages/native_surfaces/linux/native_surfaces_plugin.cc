#include "include/native_surfaces/native_surfaces_plugin.h"
#include "../common/dh_loader.h"
#include "../common/dh_presentation.h"
#include <atomic>
#include <epoxy/egl.h>
#include <thread>
#include <cstring>
#include <map>

struct _DhTexture;
typedef struct _DhTexture DhTexture;
struct EnginePool {
 DhRetirementFence<DhTexture> retired;
 DhRendererHealth renderer_health;
 DhRasterBacking backing; // Raster thread only, bounded to one 128MiB allocation.
 std::atomic<size_t> objects{0};
 std::atomic<bool> shutdown{false};
 std::mutex context_mutex;
 EGLDisplay display=EGL_NO_DISPLAY;
 EGLContext cleanup_context=EGL_NO_CONTEXT;
 std::thread::id raster_thread;
 bool EnsureCleanupContext() {
  std::lock_guard<std::mutex> lock(context_mutex);
  raster_thread=std::this_thread::get_id();
  if(cleanup_context!=EGL_NO_CONTEXT)return true;
  const EGLDisplay current_display=eglGetCurrentDisplay();
  const EGLContext render_context=eglGetCurrentContext();
  // Pinned standard Linux Flutter uses EGL GLES2. Desktop GL, non-EGL and
  // custom merged task runners are unsupported rather than guessed compatible.
  if(current_display==EGL_NO_DISPLAY||render_context==EGL_NO_CONTEXT||eglQueryAPI()!=EGL_OPENGL_ES_API)return false;
  EGLint config_id=0,version=0;
  if(!eglQueryContext(current_display,render_context,EGL_CONFIG_ID,&config_id)||
     !eglQueryContext(current_display,render_context,EGL_CONTEXT_CLIENT_VERSION,&version)||version<2)return false;
  const EGLint attributes[]={EGL_CONFIG_ID,config_id,EGL_NONE};
  EGLConfig config=nullptr;EGLint count=0;
  if(!eglChooseConfig(current_display,attributes,&config,1,&count)||count!=1)return false;
  const EGLint context_attributes[]={EGL_CONTEXT_CLIENT_VERSION,version,EGL_NONE};
  const EGLContext shared=eglCreateContext(current_display,config,render_context,context_attributes);
  if(shared==EGL_NO_CONTEXT)return false;
  display=current_display;cleanup_context=shared;return true;
 }
 bool CollectAfterRasterFence(uint64_t ticket) {
  std::lock_guard<std::mutex> lock(context_mutex);
  if(shutdown.load())return false;
  // The pinned Picture.toImage fence relies on a dedicated raster runner. Do
  // not reclaim on an observed custom merged platform/raster thread.
  if(cleanup_context!=EGL_NO_CONTEXT&&raster_thread==std::this_thread::get_id())return false;
  if(cleanup_context==EGL_NO_CONTEXT) {
   // copy_pixels refuses uploads until a cleanup context exists, so no base
   // GL texture can have been allocated on this path.
   retired.CompleteThrough(ticket);return true;
  }
  const EGLenum previous_api=eglQueryAPI();
  const EGLDisplay previous_display=eglGetCurrentDisplay();
  const EGLContext previous_context=eglGetCurrentContext();
  const EGLSurface previous_draw=eglGetCurrentSurface(EGL_DRAW);
  const EGLSurface previous_read=eglGetCurrentSurface(EGL_READ);
  if(!eglBindAPI(EGL_OPENGL_ES_API))return false;
  if(!eglMakeCurrent(display,EGL_NO_SURFACE,EGL_NO_SURFACE,cleanup_context)) {
   eglBindAPI(previous_api);return false; // Context loss retains every owner.
  }
  // Only tickets covered by the completed FIFO fence can be destroyed. Their
  // base dispose GL calls run in our owned context sharing the render resources.
  retired.CompleteThrough(ticket);
  bool restored;
  if(previous_display!=EGL_NO_DISPLAY&&previous_context!=EGL_NO_CONTEXT) {
   eglBindAPI(previous_api);
   restored=eglMakeCurrent(previous_display,previous_draw,previous_read,previous_context)==EGL_TRUE;
  } else {
   restored=eglMakeCurrent(display,EGL_NO_SURFACE,EGL_NO_SURFACE,EGL_NO_CONTEXT)==EGL_TRUE;
   eglBindAPI(previous_api);
  }
  if(!restored)eglMakeCurrent(display,EGL_NO_SURFACE,EGL_NO_SURFACE,EGL_NO_CONTEXT);
  return restored;
 }
 void ShutdownFence() {
  std::lock_guard<std::mutex> lock(context_mutex);
  shutdown.store(true);retired.Complete();
  // Flutter has terminated its EGL display; destroy may report an already-lost
  // context. No GL deletion is attempted after that engine shutdown boundary.
  if(cleanup_context!=EGL_NO_CONTEXT) {
   if(eglGetCurrentContext()==cleanup_context)eglMakeCurrent(display,EGL_NO_SURFACE,EGL_NO_SURFACE,EGL_NO_CONTEXT);
   eglDestroyContext(display,cleanup_context);cleanup_context=EGL_NO_CONTEXT;
  }
 }
};
struct TextureState {
 std::shared_ptr<EnginePool> pool;
 DhPresentation presentation;
 uint32_t session;
 TextureState(std::shared_ptr<EnginePool> p,uint32_t s):pool(std::move(p)),session(s){++pool->objects;}
 ~TextureState(){--pool->objects;}
};
struct _DhTexture {FlPixelBufferTexture parent_instance;TextureState* state;};
struct _DhTextureClass {FlPixelBufferTextureClass parent_class;};
typedef struct _DhTextureClass DhTextureClass;
G_DEFINE_TYPE(DhTexture,dh_texture,fl_pixel_buffer_texture_get_type())

static gboolean copy_pixels(FlPixelBufferTexture* base,const uint8_t** buffer,uint32_t* width,uint32_t* height,GError** error) {
 auto* texture=reinterpret_cast<DhTexture*>(base);
 auto& state=*texture->state;
 // Retired owners are held across borrowed lookup -> callback -> upload.
 // Only the explicit post-unregister raster FIFO fence permits collection.
 auto snapshot=state.presentation.ForUpload();
 const unsigned char* pixels=nullptr;
 const bool prepared=DhPrepareRaster(state.pool->renderer_health,
  [&]{return state.pool->EnsureCleanupContext();},
  [&]{pixels=state.pool->backing.Stage(*snapshot);return pixels!=nullptr;});
 if(!prepared) {
  state.presentation.Clear();
  const auto failure=state.pool->renderer_health.PublicError();
  g_set_error_literal(error,g_quark_from_static_string("deskharbor"),1,failure.message);
  return FALSE;
 }
 *buffer=pixels;*width=snapshot->width();*height=snapshot->height();
 // Rust memory is no longer read after Stage. The engine-scoped backing stays
 // unchanged through the following GL upload, even if UI disposes this texture.
 return TRUE;
}
static void dh_texture_dispose(GObject* object) {
 auto* texture=reinterpret_cast<DhTexture*>(object);
 if(texture->state && texture->state->pool->shutdown.load()) {
  // Engine Shutdown already destroyed its GL context and its GL resources.
  // Pinned FlPixelBufferTexture::dispose only deletes its GL texture then chains
  // to GObject. Do not issue that GL call without a context during final drain.
  auto* grandparent=G_OBJECT_CLASS(g_type_class_peek_parent(dh_texture_parent_class));
  if(grandparent->dispose)grandparent->dispose(object);
 } else G_OBJECT_CLASS(dh_texture_parent_class)->dispose(object);
}
static void dh_texture_finalize(GObject* object) {
 delete reinterpret_cast<DhTexture*>(object)->state;
 G_OBJECT_CLASS(dh_texture_parent_class)->finalize(object);
}
static void dh_texture_class_init(DhTextureClass* klass) {
 FL_PIXEL_BUFFER_TEXTURE_CLASS(klass)->copy_pixels=copy_pixels;
 G_OBJECT_CLASS(klass)->dispose=dh_texture_dispose;
 G_OBJECT_CLASS(klass)->finalize=dh_texture_finalize;
}
static void dh_texture_init(DhTexture* texture){texture->state=nullptr;}
struct Plugin {
 FlTextureRegistrar* registrar;
 DhApi api{};
 std::shared_ptr<EnginePool> pool=std::make_shared<EnginePool>();
 std::map<int64_t,DhTexture*> textures;
};
struct RemovedTexture {uint64_t ticket;bool unregistered;};
static RemovedTexture remove_texture(Plugin* plugin,std::map<int64_t,DhTexture*>::iterator it) {
 auto owner=std::shared_ptr<DhTexture>(it->second,[](DhTexture* texture){g_object_unref(texture);});
 owner->state->presentation.Clear(); // Never mutate raster backing on UI.
 fl_texture_registrar_mark_texture_frame_available(plugin->registrar,FL_TEXTURE(owner.get()));
 const bool removed=fl_texture_registrar_unregister_texture(plugin->registrar,FL_TEXTURE(owner.get()));
 plugin->textures.erase(it);
 if(plugin->pool->shutdown.load())return {0,true};
 // Failure remains noncollectable until engine Shutdown; a later ticket cannot
 // authorize destruction while the external engine registration might survive.
 const auto ticket=plugin->pool->retired.Retire(std::move(owner),removed);
 return {ticket,removed};
}
static void destroy(gpointer data) {
 auto* plugin=static_cast<Plugin*>(data);
 while(!plugin->textures.empty())remove_texture(plugin,plugin->textures.begin());
 g_object_unref(plugin->registrar);delete plugin;
}
static bool argument(FlMethodCall* call,const char* key,int64_t* id) {
 auto* args=fl_method_call_get_args(call);
 if(!args||fl_value_get_type(args)!=FL_VALUE_TYPE_MAP)return false;
 auto* value=fl_value_lookup_string(args,key);
 if(!value||fl_value_get_type(value)!=FL_VALUE_TYPE_INT)return false;
 *id=fl_value_get_int(value);return true;
}
static void method(FlMethodChannel*,FlMethodCall* call,gpointer data) {
 auto* plugin=static_cast<Plugin*>(data);
 const char* name=fl_method_call_get_name(call);
 int64_t id;const char* code=nullptr;const char* message=nullptr;
 if(strcmp(name,"collectDisposedTextures")==0) {
  if(!argument(call,"ticket",&id)||id<=0||static_cast<uint64_t>(id)>plugin->pool->retired.LastIssued()) {
   fl_method_call_respond_error(call,"bad_arguments","Valid disposal fence ticket required",nullptr,nullptr);return;
  }
  if(!plugin->pool->CollectAfterRasterFence(static_cast<uint64_t>(id))) {
   fl_method_call_respond_error(call,"cleanup_unavailable","Native cleanup context lost or unsupported runner; close this session and restart the application",nullptr,nullptr);return;
  }
  const auto renderer_error=plugin->pool->renderer_health.PublicError();
  if(renderer_error.code[0]) {
   fl_method_call_respond_error(call,renderer_error.code,renderer_error.message,nullptr,nullptr);return;
  }
  fl_method_call_respond_success(call,nullptr,nullptr);return;
 }
 if(strcmp(name,"createTexture")==0) {
  if(!argument(call,"sessionId",&id)||id<0||id>UINT32_MAX) {
   code="bad_arguments";message="sessionId must be uint32";
  } else if(plugin->pool->shutdown.load()) {
   code="texture_unavailable";message="Flutter view engine is unavailable";
  } else if(plugin->pool->renderer_health.failed()) {
   const auto failure=plugin->pool->renderer_health.PublicError();code=failure.code;message=failure.message;
  } else {
   // Registration precedes Dart/FRB initialization. Resolve the exact packaged
   // library lazily and retry a failed lookup at every create request.
   if(!plugin->api.acquire||!plugin->api.release)plugin->api=dh_load_api();
   if(!plugin->api.acquire||!plugin->api.release) {
    code="library_unavailable";message="Bundled libdh_ffi.so frame ABI unavailable";
   } else if(plugin->pool->objects.load()>=8) {
    code="texture_unavailable";message="Native texture capacity reached";
   } else {
    auto* texture=reinterpret_cast<DhTexture*>(g_object_new(dh_texture_get_type(),nullptr));
    texture->state=new TextureState(plugin->pool,static_cast<uint32_t>(id));
    if(!fl_texture_registrar_register_texture(plugin->registrar,FL_TEXTURE(texture))) {
     g_object_unref(texture);code="texture_unavailable";message="Texture registration failed";
    } else {
     auto texture_id=fl_texture_get_id(FL_TEXTURE(texture));plugin->textures[texture_id]=texture;
     g_autoptr(FlValue) value=fl_value_new_int(texture_id);
     fl_method_call_respond_success(call,value,nullptr);return;
    }
   }
  }
 } else if(strcmp(name,"present")==0||strcmp(name,"disposeTexture")==0) {
  if(!argument(call,"textureId",&id)) {code="bad_arguments";message="textureId required";}
  else {
   auto it=plugin->textures.find(id);
   if(it==plugin->textures.end()) {code="texture_unavailable";message="Unknown texture";}
   else if(strcmp(name,"disposeTexture")==0) {
    const auto removed=remove_texture(plugin,it);
    if(!removed.unregistered) {
     fl_method_call_respond_error(call,"cleanup_unavailable","Native texture unregister failed; restart the application to complete cleanup",nullptr,nullptr);return;
    }
    g_autoptr(FlValue) ticket=fl_value_new_int(static_cast<int64_t>(removed.ticket));
    fl_method_call_respond_success(call,ticket,nullptr);return;
   } else {
    auto& state=*it->second->state;
    const auto result=state.presentation.Present(plugin->api.acquire,plugin->api.release,state.session,&plugin->pool->renderer_health);
    // Missing data publishes an actual transparent clear, never a second acquire.
    if(!fl_texture_registrar_mark_texture_frame_available(plugin->registrar,FL_TEXTURE(it->second))) {
     code="texture_unavailable";message="Texture is unavailable";
    } else {
     const auto failure=DhPresentationError(result,&plugin->pool->renderer_health);
     if(failure.code[0]){code=failure.code;message=failure.message;}
     else {fl_method_call_respond_success(call,nullptr,nullptr);return;}
    }
   }
  }
 } else {fl_method_call_respond_not_implemented(call,nullptr);return;}
 fl_method_call_respond_error(call,code,message,nullptr,nullptr);
}
void native_surfaces_plugin_register_with_registrar(FlPluginRegistrar* registrar) {
 auto* plugin=new Plugin;
 plugin->registrar=FL_TEXTURE_REGISTRAR(g_object_ref(fl_plugin_registrar_get_texture_registrar(registrar)));
 auto* view=fl_plugin_registrar_get_view(registrar);
 auto* engine=view?fl_view_get_engine(view):nullptr;
 if(engine) {
  auto* retained_pool=new std::shared_ptr<EnginePool>(plugin->pool);
  g_object_weak_ref(G_OBJECT(engine),[](gpointer data,GObject*) {
   auto* pool=static_cast<std::shared_ptr<EnginePool>*>(data);
   // Pinned FlEngine::dispose synchronously calls Shutdown before messenger and
   // registrar teardown. Weak finalization is the final raster completion fence.
   (*pool)->ShutdownFence();delete pool;
  },retained_pool);
 } else plugin->pool->shutdown.store(true);
 g_autoptr(FlStandardMethodCodec) codec=fl_standard_method_codec_new();
 g_autoptr(FlMethodChannel) channel=fl_method_channel_new(fl_plugin_registrar_get_messenger(registrar),"deskharbor/native_surfaces",FL_METHOD_CODEC(codec));
 fl_method_channel_set_method_call_handler(channel,method,plugin,destroy);
}
