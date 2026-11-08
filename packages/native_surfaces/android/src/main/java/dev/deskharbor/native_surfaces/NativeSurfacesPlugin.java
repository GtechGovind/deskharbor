package dev.deskharbor.native_surfaces;
import androidx.annotation.NonNull;
import io.flutter.embedding.engine.plugins.FlutterPlugin;
import io.flutter.plugin.common.MethodCall;
import io.flutter.plugin.common.MethodChannel;
import io.flutter.view.TextureRegistry;
import java.util.HashMap;
import java.util.Map;
public final class NativeSurfacesPlugin implements FlutterPlugin, MethodChannel.MethodCallHandler {
 private static final boolean loaded;
 static { boolean ok;try {System.loadLibrary("native_surfaces");ok=true;}catch(UnsatisfiedLinkError e){ok=false;}loaded=ok; }
 private static native int nativePresent(long session,TextureRegistry.SurfaceProducer producer);
 private static native boolean nativeAvailable();
 private MethodChannel channel;
 private TextureRegistry registry;
 private static final class Entry implements TextureRegistry.SurfaceProducer.Callback {
  final long session;final TextureRegistry.SurfaceProducer producer;boolean available=true;
  Entry(long s,TextureRegistry.SurfaceProducer p){session=s;producer=p;p.setCallback(this);}
  @Override public void onSurfaceAvailable(){available=true;}
  @Override public void onSurfaceCleanup(){available=false;}
  void release(){available=false;producer.setCallback(null);producer.release();}
 }
 private final Map<Long,Entry> surfaces=new HashMap<>();
 @Override public void onAttachedToEngine(@NonNull FlutterPluginBinding binding){registry=binding.getTextureRegistry();channel=new MethodChannel(binding.getBinaryMessenger(),"deskharbor/native_surfaces");channel.setMethodCallHandler(this);}
 private static Long number(MethodCall call,String key){if(!(call.arguments instanceof Map))return null;Object value=call.argument(key);return value instanceof Integer||value instanceof Long?((Number)value).longValue():null;}
 @Override public void onMethodCall(@NonNull MethodCall call,@NonNull MethodChannel.Result result){
  if(call.method.equals("createTexture")){
   Long sid=number(call,"sessionId");if(sid==null||sid<0||sid>0xffffffffL){result.error("bad_arguments","sessionId must be uint32",null);return;}
   if(!loaded||!nativeAvailable()){result.error("library_unavailable","Shared libdh_ffi.so frame ABI unavailable",null);return;}
   try {TextureRegistry.SurfaceProducer producer=registry.createSurfaceProducer();Entry entry=new Entry(sid,producer);surfaces.put(producer.id(),entry);result.success(producer.id());}catch(RuntimeException e){result.error("texture_unavailable","SurfaceProducer registration failed",null);}return;
  }
  if(!call.method.equals("present")&&!call.method.equals("disposeTexture")){result.notImplemented();return;}
  Long id=number(call,"textureId");Entry entry=surfaces.get(id);
  if(entry==null){result.error("texture_unavailable","Unknown texture",null);return;}
  if(call.method.equals("disposeTexture")){surfaces.remove(id);entry.release();result.success(null);return;}
  if(!entry.available){result.error("surface_unavailable","Android surface is suspended",null);return;}
  int status=nativePresent(entry.session,entry.producer);
  if(status==0)result.success(null);else result.error(status==1?"frame_unavailable":"surface_unavailable",status==1?"No decoded frame":"Native RGBA surface rendering failed",null);
 }
 @Override public void onDetachedFromEngine(@NonNull FlutterPluginBinding binding){channel.setMethodCallHandler(null);for(Entry entry:surfaces.values())entry.release();surfaces.clear();registry=null;channel=null;}
}
