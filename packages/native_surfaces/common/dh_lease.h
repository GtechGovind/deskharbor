#ifndef DH_LEASE_H
#define DH_LEASE_H
#include <atomic>
#include "dh_frame.h"
class DhLease {
 std::atomic<void*> value_;DhRelease release_;
public:
 DhLease(void* value,DhRelease release):value_(value),release_(release){}
 DhLease(const DhLease&)=delete;DhLease& operator=(const DhLease&)=delete;
 void Complete(){if(void* value=value_.exchange(nullptr)){release_(value);}}
 bool completed() const {return value_.load()==nullptr;}
 // Only the serialized renderer acquisition callback may Reset a completed lease.
 void Reset(void* value){value_.store(value);}
 ~DhLease(){Complete();}
};
#endif
