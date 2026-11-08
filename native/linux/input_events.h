#pragma once
#include <linux/input.h>
static inline int f24_frames(struct input_event events[4]) {
    events[0] = (struct input_event){.type=EV_KEY,.code=KEY_F24,.value=1};
    events[1] = (struct input_event){.type=EV_SYN,.code=SYN_REPORT};
    events[2] = (struct input_event){.type=EV_KEY,.code=KEY_F24,.value=0};
    events[3] = (struct input_event){.type=EV_SYN,.code=SYN_REPORT};
    return 4;
}
