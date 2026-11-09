#include "../../common/dh_presentation.h"
#include <cstdio>
#include <cstring>
#define CHECK(c) do {if(!(c)){fprintf(stderr,"Check failed at line %d: %s\n",__LINE__,#c);return 1;}} while(0)
static int acquired=0,released=0;
static DhRendererHealth* during_copy=nullptr;
static bool copy_and_fail(const unsigned char* data,size_t length,std::vector<unsigned char>& out){const bool copied=DhCopyOwnedRgba(data,length,out);during_copy->RecordFailure(DhRenderFailure::egl_setup);return copied;}
static bool acquire(unsigned int,DhFrame* out){++acquired;auto* bytes=new unsigned char[4]{255,0,0,255};*out={bytes,4,1,1,4,bytes};return true;}
static void release(void* lease){++released;delete[] static_cast<unsigned char*>(lease);}
int main(){
 DhRendererHealth health;DhPresentation presentation;
 CHECK(presentation.Present(acquire,release,7,&health)==DhPresentResult::ready);
 CHECK(acquired==1&&released==1);
 bool copy_called=false;
 CHECK(!DhPrepareRaster(health,[]{return false;},[&]{copy_called=true;return true;}));
 CHECK(!copy_called);CHECK(health.failed());
 // Exact public-presentation result and error adapter used by the Linux method
 // channel: injected EGL-create failure cannot produce another success reply.
 const auto result=presentation.Present(acquire,release,7,&health);
 CHECK(result==DhPresentResult::renderer_failed);CHECK(acquired==1);
 CHECK(presentation.ForUpload()->empty());
 const auto error=DhPresentationError(result,&health);
 CHECK(strcmp(error.code,"renderer_initialization_failed")==0);
 CHECK(strstr(error.message,"restart")!=nullptr);
 CHECK(strcmp(DhPresentationError(DhPresentResult::ready,&health).code,"renderer_initialization_failed")==0);
 health.RecordFailure(DhRenderFailure::copy_failed);CHECK(strcmp(health.PublicError().code,"renderer_initialization_failed")==0);
 DhRendererHealth copy_failure;DhPresentation second;
 CHECK(!DhPrepareRaster(copy_failure,[]{return true;},[]{return false;}));
 CHECK(second.Present(acquire,release,8,&copy_failure)==DhPresentResult::renderer_failed);
 CHECK(strcmp(DhPresentationError(DhPresentResult::renderer_failed,&copy_failure).code,"renderer_copy_failed")==0);
 // A surface never asked to render has no error; initialization failure remains
 // distinct even if neither case ever allocated a GL texture.
 DhRendererHealth never_rendered;CHECK(!never_rendered.failed());CHECK(!never_rendered.PublicError().code[0]);
 CHECK(health.PublicError().code[0]);
 // A failure recorded while native snapshot copying is in progress must also
 // win over an otherwise ready result before publication to the method caller.
 DhRendererHealth racing_health;during_copy=&racing_health;
 DhPresentation racing(copy_and_fail);
 CHECK(racing.Present(acquire,release,9,&racing_health)==DhPresentResult::renderer_failed);
 CHECK(racing.ForUpload()->empty());

}
