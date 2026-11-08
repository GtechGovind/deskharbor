#include "input_events.h"
#include <stdio.h>
int main(void) {
    struct input_event emitted[4] = {0};
    const unsigned expected[][3] = {{1,194,1}, {0,0,0}, {1,194,0}, {0,0,0}};
    if (f24_frames(emitted) != 4) { fputs("missing key and release frames\n",stderr); return 1; }
    for (int i=0; i<4; i++)
        if (emitted[i].type!=expected[i][0] || emitted[i].code!=expected[i][1] ||
            emitted[i].value!=(int)expected[i][2]) return 1;
    return 0;
}
