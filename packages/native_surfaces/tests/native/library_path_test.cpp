#include "../../common/dh_library_path.h"
#include <cstdio>
#define CHECK(c) do {if(!(c)){fprintf(stderr,"Check failed at line %d: %s\n",__LINE__,#c);return 1;}} while(0)
int main(){char path[256];CHECK(dh_bundle_library_path("/relocated bundle/client",false,path,sizeof(path)));
 CHECK(strcmp(path,"/relocated bundle/lib/libdh_ffi.so")==0);
 CHECK(dh_bundle_library_path("/Test.app/Contents/MacOS/client",true,path,sizeof(path)));
 CHECK(strcmp(path,"/Test.app/Contents/MacOS/../Frameworks/libdh_ffi.dylib")==0);
 char short_path[4]={1,2,3,4};CHECK(!dh_bundle_library_path("/a/client",false,short_path,3));CHECK(short_path[3]==4);
 CHECK(!dh_bundle_library_path("client",false,path,sizeof(path)));
}
