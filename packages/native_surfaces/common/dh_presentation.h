#ifndef DH_PRESENTATION_H
#define DH_PRESENTATION_H
#include "dh_frame.h"
#include <array>
#include <cstring>
#include <memory>
#include <mutex>
#include <vector>
class DhSnapshot {
 DhFrame frame_{};DhRelease release_=nullptr;
 std::array<unsigned char,4> empty_{};
public:
 DhSnapshot()=default;
 DhSnapshot(DhFrame frame,DhRelease release):frame_(frame),release_(release){}
 ~DhSnapshot(){if(frame_.lease)release_(frame_.lease);}
 DhSnapshot(const DhSnapshot&)=delete;DhSnapshot& operator=(const DhSnapshot&)=delete;
 bool empty() const{return !frame_.lease;}
 const unsigned char* data() const{return empty()?empty_.data():frame_.data;}
 unsigned int width() const{return empty()?1:frame_.width;}
 unsigned int height() const{return empty()?1:frame_.height;}
 size_t length() const{return empty()?4:frame_.length;}
};
enum class DhPresentResult {ready,unavailable,invalid};
class DhPresentation {
 std::mutex mutex_;
 const std::shared_ptr<DhSnapshot> empty_=std::make_shared<DhSnapshot>();
 std::shared_ptr<DhSnapshot> current_=empty_;
public:
 DhPresentResult Present(DhAcquire acquire,DhRelease release,unsigned int session){
  DhFrame frame{};if(!acquire(session,&frame)){Clear();return DhPresentResult::unavailable;}
  if(!dh_frame_valid(&frame)||frame.length>128u*1024u*1024u){release(frame.lease);Clear();return DhPresentResult::invalid;}
  auto snapshot=std::make_shared<DhSnapshot>(frame,release);std::lock_guard<std::mutex> lock(mutex_);current_=std::move(snapshot);return DhPresentResult::ready;
 }
 std::shared_ptr<DhSnapshot> ForUpload(){std::lock_guard<std::mutex> lock(mutex_);return current_;}
 void Clear(){std::lock_guard<std::mutex> lock(mutex_);current_=empty_;}
};
/* One staging allocation per engine, used only on its serial raster thread.
 * glTexImage2D consumes these client bytes before the next callback begins. */
class DhRasterBacking {
 std::vector<unsigned char> pixels_;
public:
 const unsigned char* Stage(const DhSnapshot& snapshot) {
  if(snapshot.length()>128u*1024u*1024u)return nullptr;
  if(snapshot.length()>pixels_.capacity()) {
   std::vector<unsigned char> replacement(snapshot.length());pixels_.swap(replacement);
  } else pixels_.resize(snapshot.length());
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
