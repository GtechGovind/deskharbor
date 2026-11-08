# JNI uses these stable method and interface names.
-keepclasseswithmembernames,includedescriptorclasses class dev.deskharbor.native_surfaces.NativeSurfacesPlugin { native <methods>; }
-keep interface io.flutter.view.TextureRegistry$SurfaceProducer { *; }
