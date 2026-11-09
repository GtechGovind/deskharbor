#include "../../common/dh_presentation.h"
#include <cstdio>
#include <cstring>
#define CHECK(c) do {if(!(c)){fprintf(stderr,"Check failed at line %d: %s\n",__LINE__,#c);return 1;}} while(0)
static int acquires=0,releases=0;static bool available=true;
struct Pixels {unsigned char bytes[8]={255,0,0,255,0,255,0,255};};
static bool acquire(unsigned int,DhFrame* out){++acquires;if(!available){*out={};return false;}auto* p=new Pixels;*out={p->bytes,8,2,1,8,p};return true;}
static void release(void* p){if(p){++releases;delete static_cast<Pixels*>(p);}}
int main(){
 DhPresentation presentation;CHECK(presentation.Present(acquire,release,7)==DhPresentResult::ready);
 available=false;auto raster=presentation.ForUpload();CHECK(acquires==1);CHECK(!raster->empty());CHECK(raster->data()[0]==255);
 CHECK(presentation.Present(acquire,release,7)==DhPresentResult::unavailable);
 auto cleared=presentation.ForUpload();CHECK(cleared->empty());CHECK(cleared->width()==1&&cleared->height()==1);CHECK(cleared->data()[0]==0&&cleared->data()[3]==0);
 CHECK(releases==1);raster.reset();CHECK(releases==1);CHECK(acquires==2);
 // Borrowed lookup precedes unregister; its raw object survives both the
 // callback and upload. A later callback by itself is NOT a disposal fence.
 auto owner=std::make_shared<DhPresentation>();auto weak=std::weak_ptr<DhPresentation>(owner);
 DhPresentation* borrowed=owner.get();DhRetirementFence<DhPresentation> fence;
 owner->Clear();const auto ticket_a=fence.Retire(std::move(owner));
 CHECK(!weak.expired());CHECK(borrowed->ForUpload()->empty());
 auto second=std::make_shared<DhPresentation>();
 available=true;CHECK(second->Present(acquire,release,9)==DhPresentResult::ready);
 auto pending=second->ForUpload();DhRasterBacking backing;const auto* upload=backing.Stage(*pending);
 CHECK(upload&&upload[0]==255);
 // Dispose AFTER copy and BEFORE upload; UI cannot mutate the shared backing.
 second->Clear();auto second_weak=std::weak_ptr<DhPresentation>(second);
 const auto ticket_b=fence.Retire(std::move(second));pending.reset();
 CHECK(upload[0]==255&&upload[7]==255);CHECK(!second_weak.expired());
 // The blank offscreen raster task for A finished, but B retired after that
 // task was posted. A's acknowledgement must not collect B.
 fence.CompleteThrough(ticket_a);CHECK(weak.expired());CHECK(!second_weak.expired());
 fence.CompleteThrough(ticket_b);CHECK(second_weak.expired());
 DhPresentation other;const auto* empty=backing.Stage(*other.ForUpload());
 CHECK(empty&&empty[0]==0&&empty[3]==0);CHECK(backing.capacity()==8);
 // Failed unregister/context/fence paths keep owners until real shutdown.
 auto failed=std::make_shared<DhPresentation>();auto failed_weak=std::weak_ptr<DhPresentation>(failed);
 const auto failed_ticket=fence.Retire(std::move(failed),false);
 fence.CompleteThrough(failed_ticket);CHECK(!failed_weak.expired());fence.Complete();CHECK(failed_weak.expired());
 const int released_before=releases;
 std::vector<std::weak_ptr<DhPresentation>> retired;
 for(unsigned int i=0;i<50;i++) {
  auto texture=std::make_shared<DhPresentation>();
  CHECK(texture->Present(acquire,release,i)==DhPresentResult::ready);
  auto snapshot=texture->ForUpload();const auto* bytes=backing.Stage(*snapshot);
  texture->Clear();retired.emplace_back(texture);const auto ticket=fence.Retire(std::move(texture));snapshot.reset();
  CHECK(bytes[0]==255&&bytes[7]==255);CHECK(backing.capacity()==8);
  fence.CompleteThrough(ticket); // Models the actual FIFO fence, after upload.
 }
 CHECK(releases-released_before==50);
 for(const auto& object:retired)CHECK(object.expired());
 fence.Complete();
}
