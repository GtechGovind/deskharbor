#include "../../common/dh_copy.h"
#include <cstdio>
#define CHECK(condition) do {if(!(condition)){fprintf(stderr,"Check failed at line %d: %s\n",__LINE__,#condition);return 1;}} while(0)
#include <cstring>
int main(){
 // Distinct red/green/blue/white corners expose channel swaps and row inversion.
 const unsigned char src[]={255,0,0,255, 0,255,0,255, 0,0,255,255, 255,255,255,255};
 DhFrame f{src,16,2,2,8,(void*)src};unsigned char dst[24];memset(dst,77,sizeof(dst));
 CHECK(dh_copy_rgba(&f,dst,12,sizeof(dst),false));
 CHECK(memcmp(dst,src,8)==0);CHECK(memcmp(dst+12,src+8,8)==0);CHECK(dst[8]==77&&dst[23]==77);
 CHECK(dh_copy_rgba(&f,dst,12,sizeof(dst),true));
 const unsigned char bgra[]={0,0,255,255,0,255,0,255};CHECK(memcmp(dst,bgra,8)==0);CHECK(dst[12]==255&&dst[14]==0);
 CHECK(!dh_copy_rgba(&f,dst,7,sizeof(dst),false));CHECK(!dh_copy_rgba(&f,dst,12,23,false));
 f.length=15;CHECK(!dh_copy_rgba(&f,dst,12,sizeof(dst),false));f.length=16;f.stride=7;CHECK(!dh_copy_rgba(&f,dst,12,sizeof(dst),false));
 return 0;
}
