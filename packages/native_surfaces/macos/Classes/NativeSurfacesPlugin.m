#import "NativeSurfacesPlugin.h"
#import <CoreVideo/CoreVideo.h>
#import "../../common/dh_loader.h"
#import "../../common/dh_copy.h"
static BOOL DHInteger(id value) {
 return [value isKindOfClass:[NSNumber class]] && CFGetTypeID((__bridge CFTypeRef)value)!=CFBooleanGetTypeID() && !CFNumberIsFloatType((__bridge CFNumberRef)value);
}
@interface DHTexture : NSObject<FlutterTexture>
@property(nonatomic) uint32_t session;
@property(nonatomic) CVPixelBufferRef pixels;
@end
@implementation DHTexture
- (CVPixelBufferRef)copyPixelBuffer { @synchronized(self) {return self.pixels ? CVPixelBufferRetain(self.pixels) : NULL;} }
- (void)dealloc {if(_pixels)CVPixelBufferRelease(_pixels);}
@end
@interface NativeSurfacesPlugin ()
@property(nonatomic,strong) NSObject<FlutterTextureRegistry>* registry;
@property(nonatomic,strong) NSMutableDictionary<NSNumber*,DHTexture*>* surfaces;
@property(nonatomic) DhApi api;
@end
@implementation NativeSurfacesPlugin
+ (void)registerWithRegistrar:(NSObject<FlutterPluginRegistrar>*)registrar {
 NativeSurfacesPlugin* plugin=[NativeSurfacesPlugin new];plugin.registry=registrar.textures;plugin.surfaces=[NSMutableDictionary new];plugin.api=dh_load_api();
 FlutterMethodChannel* channel=[FlutterMethodChannel methodChannelWithName:@"deskharbor/native_surfaces" binaryMessenger:registrar.messenger];
 [registrar addMethodCallDelegate:plugin channel:channel];
}
- (void)handleMethodCall:(FlutterMethodCall*)call result:(FlutterResult)result {
 NSDictionary* args=[call.arguments isKindOfClass:[NSDictionary class]]?call.arguments:nil;
 NSString* method=call.method;
 if([method isEqualToString:@"createTexture"]){
  NSNumber* sid=args[@"sessionId"];
  if(!DHInteger(sid)||sid.longLongValue<0||sid.unsignedLongLongValue>UINT32_MAX){result([FlutterError errorWithCode:@"bad_arguments" message:@"sessionId must be uint32" details:nil]);return;}
  if(!_api.acquire||!_api.release){result([FlutterError errorWithCode:@"library_unavailable" message:@"libdh_ffi.dylib frame ABI unavailable" details:nil]);return;}
  DHTexture* t=[DHTexture new];t.session=sid.unsignedIntValue;int64_t tid=[_registry registerTexture:t];
  if(tid<0){result([FlutterError errorWithCode:@"texture_unavailable" message:@"Texture registration failed" details:nil]);return;}
  _surfaces[@(tid)]=t;result(@(tid));return;
 }
 if(![method isEqualToString:@"present"]&&![method isEqualToString:@"disposeTexture"]){result(FlutterMethodNotImplemented);return;}
 NSNumber* tid=args[@"textureId"];DHTexture* t=DHInteger(tid)?_surfaces[tid]:nil;
 if(!t){result([FlutterError errorWithCode:@"texture_unavailable" message:@"Unknown texture" details:nil]);return;}
 if([method isEqualToString:@"disposeTexture"]){[_registry unregisterTexture:tid.longLongValue];[_surfaces removeObjectForKey:tid];result(nil);return;}
 DhFrame f={0};if(!_api.acquire(t.session,&f)){result([FlutterError errorWithCode:@"frame_unavailable" message:@"No decoded frame" details:nil]);return;}
 if(!dh_frame_valid(&f)){_api.release(f.lease);result([FlutterError errorWithCode:@"invalid_frame" message:@"Malformed RGBA frame" details:nil]);return;}
 CVPixelBufferRef pixel=NULL;
 CVReturn status=CVPixelBufferCreate(kCFAllocatorDefault,f.width,f.height,kCVPixelFormatType_32BGRA,(__bridge CFDictionaryRef)@{(id)kCVPixelBufferIOSurfacePropertiesKey:@{}},&pixel);
 if(status==kCVReturnSuccess)status=CVPixelBufferLockBaseAddress(pixel,0);
 if(status==kCVReturnSuccess){
  uint8_t* dst=CVPixelBufferGetBaseAddress(pixel);size_t stride=CVPixelBufferGetBytesPerRow(pixel);
  // Flutter macOS consumes BGRA, with the same top-left orientation.
  if(!dh_copy_rgba(&f,dst,stride,CVPixelBufferGetDataSize(pixel),true))status=kCVReturnInvalidArgument;
  CVPixelBufferUnlockBaseAddress(pixel,0);
 }
 // Native copy has finished reading Rust memory; CVPixelBuffer owns the result.
 _api.release(f.lease);
 if(status!=kCVReturnSuccess){if(pixel)CVPixelBufferRelease(pixel);result([FlutterError errorWithCode:@"texture_unavailable" message:@"CVPixelBuffer allocation failed" details:nil]);return;}
 @synchronized(t){if(t.pixels)CVPixelBufferRelease(t.pixels);t.pixels=pixel;}
 [_registry textureFrameAvailable:tid.longLongValue];result(nil);
}
- (void)detachFromEngineForRegistrar:(NSObject<FlutterPluginRegistrar>*)registrar {for(NSNumber* tid in _surfaces)[_registry unregisterTexture:tid.longLongValue];[_surfaces removeAllObjects];}
- (void)dealloc {for(NSNumber* tid in _surfaces)[_registry unregisterTexture:tid.longLongValue];}
@end
