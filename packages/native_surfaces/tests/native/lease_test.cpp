#include "../../common/dh_lease.h"
#include <cstdio>
#define CHECK(condition) do {if(!(condition)){fprintf(stderr,"Check failed at line %d: %s\n",__LINE__,#condition);return 1;}} while(0)
#include <thread>
static std::atomic<int> releases{0};
static void release(void* p){delete static_cast<int*>(p);releases.fetch_add(1);}
int main(){
 {DhLease lease(new int(123),release);std::thread a([&]{lease.Complete();});std::thread b([&]{lease.Complete();});a.join();b.join();CHECK(lease.completed());CHECK(releases==1);
 lease.Reset(new int(456));CHECK(!lease.completed());lease.Complete();CHECK(releases==2);}
 CHECK(releases==2);
 {DhLease lease(new int(7),release);}CHECK(releases==3);
}
