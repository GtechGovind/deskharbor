#include "../../common/dh_presentation.h"
#include <cstdio>
#include <new>
#define CHECK(c) do {if(!(c)){fprintf(stderr,"Check failed at line %d: %s\n",__LINE__,#c);return 1;}} while(0)
static int acquired=0,released=0;static bool fail_copy=false;static bool throw_copy=false;static int copy_calls=0;
static bool copy(const unsigned char* data,size_t length,std::vector<unsigned char>& out){++copy_calls;if(throw_copy)throw std::bad_alloc();if(fail_copy)return false;return DhCopyOwnedRgba(data,length,out);}

struct Pixels {unsigned char bytes[8]={255,0,0,255,0,255,0,255};};
static bool oversized(unsigned int,DhFrame* out){auto* p=new Pixels;*out={p->bytes,134217732u,33554433u,1,134217732u,p};return true;}
static bool acquire(unsigned int,DhFrame* out){++acquired;auto* p=new Pixels;*out={p->bytes,8,2,1,8,p};return true;}
static void release(void* lease){++released;delete static_cast<Pixels*>(lease);}
int main(){
 DhPresentation presentation;
 CHECK(presentation.Present(acquire,release,7)==DhPresentResult::ready);
 {auto upload=presentation.ForUpload();CHECK(upload->data()[0]==255);}
 // No second present or dispose: the completed native copy/upload must not keep
 // the Rust Box/Arc alive merely to supply repeat paints.
 CHECK(released==1);
 auto repeat=presentation.ForUpload();CHECK(repeat->width()==2&&repeat->height()==1);
 CHECK(repeat->data()[0]==255&&repeat->data()[5]==255);
 CHECK(acquired==1&&released==1);
 // Allocation failure after a valid frame must clear pending pixels and release
 // that acquired lease once; no failed copy can leave the previous video shown.
 DhPresentation fallible(copy);CHECK(fallible.Present(acquire,release,8)==DhPresentResult::ready);
 const int before_failure=released;fail_copy=true;
 CHECK(fallible.Present(acquire,release,8)==DhPresentResult::copy_failed);
 CHECK(released==before_failure+1);CHECK(fallible.ForUpload()->empty());
 CHECK(fallible.ForUpload()->data()[3]==0);
 const auto failure=DhPresentationError(DhPresentResult::copy_failed);
 CHECK(failure.code[0]&&failure.message[0]);
 // A throwing allocation backend must unwind its native temporary and lease.
 fail_copy=false;throw_copy=true;const int before_throw=released;
 try {CHECK(fallible.Present(acquire,release,8)==DhPresentResult::copy_failed);}
 catch(...){fprintf(stderr,"Allocation failure escaped native presentation\n");return 1;}
 CHECK(released==before_throw+1);CHECK(fallible.ForUpload()->empty());
 // Dimensions/length are rejected before invoking the allocation/copy backend.
 const int before_copy=copy_calls;const int before_large=released;
 CHECK(fallible.Present(oversized,release,8)==DhPresentResult::invalid);
 CHECK(copy_calls==before_copy);CHECK(released==before_large+1);

}
