#ifndef NATIVE_SURFACES_PLUGIN_C_API_H
#define NATIVE_SURFACES_PLUGIN_C_API_H
#include <flutter_plugin_registrar.h>
#ifdef NATIVE_SURFACES_PLUGIN_IMPL
#define NS_EXPORT __declspec(dllexport)
#else
#define NS_EXPORT __declspec(dllimport)
#endif
#ifdef __cplusplus
extern "C" {
#endif
NS_EXPORT void NativeSurfacesPluginCApiRegisterWithRegistrar(FlutterDesktopPluginRegistrarRef registrar);
#ifdef __cplusplus
}
#endif
#endif
