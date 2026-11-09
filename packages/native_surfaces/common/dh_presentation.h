#ifndef DH_PRESENTATION_H
#define DH_PRESENTATION_H
#include "dh_frame.h"
#include "dh_lease.h"
#include "dh_renderer_health.h"
#include <array>
#include <cstring>
#include <memory>
#include <mutex>
#include <vector>
using DhOwnedCopy=bool(*)(const unsigned char*,size_t,std::vector<unsigned char>&);
static inline bool DhCopyOwnedRgba(const unsigned char* data,size_t length,std::vector<unsigned char>& out) noexcept {
 try {out.assign(data,data+length);return true;}catch(...){return false;}
}
class DhSnapshot {
 std::vector<unsigned char> pixels_;
 unsigned int width_=1,height_=1;
 std::array<unsigned char,4> empty_{};
public:
 DhSnapshot()=default;
 DhSnapshot(const DhSnapshot&)=delete;DhSnapshot& operator=(const DhSnapshot&)=delete;
 static std::shared_ptr<DhSnapshot> Copy(DhFrame frame,DhRelease release,DhOwnedCopy copy) {
  // Rust memory is read only by this synchronous native copy. Even allocation
  // failure releases the lease before returning; repeat paints own no Rust Arc.
  DhLease lease(frame.lease,release);
  if(!dh_frame_valid(&frame)||frame.length>128u*1024u*1024u||!copy)return {};
  try {
   auto snapshot=std::make_shared<DhSnapshot>();
   if(!copy(frame.data,frame.length,snapshot->pixels_)||snapshot->pixels_.size()!=frame.length)return {};
   snapshot->width_=frame.width;snapshot->height_=frame.height;return snapshot;
  }catch(...){return {};}
 }
 bool empty() const{return pixels_.empty();}
 const unsigned char* data() const{return empty()?empty_.data():pixels_.data();}
 unsigned int width() const{return width_;}
 unsigned int height() const{return height_;}
 size_t length() const{return empty()?4:pixels_.size();}
};
enum class DhPresentResult {ready,unavailable,invalid,copy_failed,renderer_failed};
class DhPresentation {
 std::mutex mutex_;
 DhOwnedCopy copy_;
 const std::shared_ptr<DhSnapshot> empty_=std::make_shared<DhSnapshot>();
 std::shared_ptr<DhSnapshot> current_=empty_;
public:
 explicit DhPresentation(DhOwnedCopy copy=DhCopyOwnedRgba):copy_(copy){}
 DhPresentResult Present(DhAcquire acquire,DhRelease release,unsigned int session,const DhRendererHealth* health=nullptr) {
  if(health&&health->failed()){Clear();return DhPresentResult::renderer_failed;}
  DhFrame frame{};if(!acquire(session,&frame)){Clear();return DhPresentResult::unavailable;}
  if(!dh_frame_valid(&frame)||frame.length>128u*1024u*1024u){release(frame.lease);Clear();return DhPresentResult::invalid;}
  auto snapshot=DhSnapshot::Copy(frame,release,copy_);
  if(!snapshot){Clear();return DhPresentResult::copy_failed;}
  if(health&&health->failed()){Clear();return DhPresentResult::renderer_failed;}
  std::lock_guard<std::mutex> lock(mutex_);current_=std::move(snapshot);return DhPresentResult::ready;
 }
 std::shared_ptr<DhSnapshot> ForUpload(){std::lock_guard<std::mutex> lock(mutex_);return current_;}
 void Clear(){std::lock_guard<std::mutex> lock(mutex_);current_=empty_;}
};
static inline DhRendererError DhPresentationError(DhPresentResult result,const DhRendererHealth* health=nullptr) {
 if(health&&health->failed())return health->PublicError();
 switch(result) {
  case DhPresentResult::unavailable:return {"frame_unavailable","No decoded frame for session"};
  case DhPresentResult::invalid:return {"invalid_frame","Malformed or oversized RGBA frame"};
  case DhPresentResult::copy_failed:return {"frame_copy_failed","Native frame copy allocation failed; lower the display resolution and retry"};
  case DhPresentResult::renderer_failed:return health?health->PublicError():DhRendererError{"renderer_unavailable","Native renderer failed; restart the application"};
  default:return {"",""};
 }
}
/* One staging allocation per engine, used only on its serial raster thread.
 * glTexImage2D consumes these client bytes before the next callback begins. */
class DhRasterBacking {
 std::vector<unsigned char> pixels_;
public:
 const unsigned char* Stage(const DhSnapshot& snapshot) {
  if(snapshot.length()>128u*1024u*1024u)return nullptr;
  try {
   if(snapshot.length()>pixels_.capacity()) {
    std::vector<unsigned char> replacement(snapshot.length());pixels_.swap(replacement);
   } else pixels_.resize(snapshot.length());
  }catch(...){return nullptr;}
  std::memcpy(pixels_.data(),snapshot.data(),snapshot.length());return pixels_.data();
 }
 size_t capacity() const {return pixels_.capacity();}
};
/* UI queues ownership only after unregister. Collection requires the explicit
 * post-unregister raster FIFO fence ticket; upload completion alone is not it. */
template<class T> class DhRetirementFence {
 std::mutex mutex_;
 uint64_t next_=0;
 struct Entry {uint64_t ticket;bool collectable;std::shared_ptr<T> owner;};
 std::vector<Entry> retired_;
public:
 uint64_t Retire(std::shared_ptr<T> value,bool collectable=true) {
  std::lock_guard<std::mutex> lock(mutex_);
  const auto ticket=++next_;retired_.push_back({ticket,collectable,std::move(value)});return ticket;
 }
 uint64_t LastIssued(){std::lock_guard<std::mutex> lock(mutex_);return next_;}
 void CompleteThrough(uint64_t ticket) {
  std::vector<Entry> completed;
  {
   std::lock_guard<std::mutex> lock(mutex_);
   for(auto it=retired_.begin();it!=retired_.end();) {
    if(it->collectable&&it->ticket<=ticket){completed.push_back(std::move(*it));it=retired_.erase(it);}else ++it;
   }
  }
 }
 void Complete(){std::vector<Entry> completed;{std::lock_guard<std::mutex> lock(mutex_);completed.swap(retired_);}}
};
#endif
