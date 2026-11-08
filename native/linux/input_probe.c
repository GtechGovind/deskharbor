#define _GNU_SOURCE
#include <errno.h>
#include <fcntl.h>
#include <linux/audit.h>
#include <linux/capability.h>
#include <linux/filter.h>
#include <linux/seccomp.h>
#include <linux/uinput.h>
#include <signal.h>
#include <stddef.h>
#include <stdlib.h>
#include <string.h>
#include <sys/auxv.h>
#include <sys/prctl.h>
#include <sys/stat.h>
#include <sys/syscall.h>
#include <sys/sysmacros.h>
#include <time.h>
#include <unistd.h>
#include "input_events.h"
#if !defined(__x86_64__) || defined(__ILP32__)
#error This one-shot input probe supports x86_64 LP64.
#endif

#define DENY BPF_STMT(BPF_RET|BPF_K,SECCOMP_RET_ERRNO|EPERM)
#define ALLOW BPF_STMT(BPF_RET|BPF_K,SECCOMP_RET_ALLOW)
#define LOAD(field) BPF_STMT(BPF_LD|BPF_W|BPF_ABS, offsetof(struct seccomp_data, field))
#define EQ(v,t,f) BPF_JUMP(BPF_JMP|BPF_JEQ|BPF_K,(v),(t),(f))
#define PERMIT(v) EQ(v,0,1),ALLOW
static int sandbox(void) {
    struct sock_filter policy[] = {
        LOAD(arch), EQ(AUDIT_ARCH_X86_64,1,0), BPF_STMT(BPF_RET|BPF_K,SECCOMP_RET_KILL_PROCESS),
        LOAD(nr), PERMIT(SYS_exit), PERMIT(SYS_exit_group), PERMIT(SYS_close),
        PERMIT(SYS_clock_nanosleep),
        EQ(SYS_write,0,4), LOAD(args[0]), EQ(3,1,0), DENY, ALLOW,
        EQ(SYS_ioctl,1,0), DENY, LOAD(args[0]), EQ(3,1,0), DENY, LOAD(args[1]),
        PERMIT(UI_SET_EVBIT), PERMIT(UI_SET_KEYBIT), PERMIT(UI_DEV_SETUP),
        PERMIT(UI_DEV_CREATE), PERMIT(UI_DEV_DESTROY), DENY
    };
    struct sock_fprog filter = {.len=sizeof policy / sizeof *policy, .filter=policy};
    return prctl(PR_SET_NO_NEW_PRIVS,1L,0L,0L,0L) || prctl(PR_SET_SECCOMP,SECCOMP_MODE_FILTER,&filter);
}
int main(int argc, char **argv) {
    if (argc != 2 || strcmp(argv[1], "--confirm-local-lab-f24")) return 2;
    struct __user_cap_header_struct h = {.version=_LINUX_CAPABILITY_VERSION_3};
    struct __user_cap_data_struct caps[2] = {0};
    if (!geteuid() || getuid()!=geteuid() || getgid()!=getegid() || getauxval(AT_SECURE) ||
        syscall(SYS_capget,&h,caps) < 0 || caps[0].permitted || caps[1].permitted ||
        caps[0].inheritable || caps[1].inheritable) return 11;
    int fd = open("/dev/uinput",O_WRONLY|O_CLOEXEC|O_NOFOLLOW|O_NONBLOCK);
    if (fd < 0) return (errno==EPERM || errno==EACCES) ? 3 : 10;
    struct stat meta;
    if (fstat(fd,&meta) || !S_ISCHR(meta.st_mode) || major(meta.st_rdev)!=10 || minor(meta.st_rdev)!=223)
        return 2;
    if (fd != 3 && dup3(fd,3,O_CLOEXEC)<0) return 11;
    if (syscall(SYS_close_range,4u,~0u,0u)<0 || prctl(PR_SET_DUMPABLE,0L)<0) return 11;
    close(0); close(1); close(2);
    alarm(8);
    if (sandbox()) return 11;
    struct uinput_setup setup = {.id={.bustype=BUS_VIRTUAL,.vendor=1,.product=1},.name="DeskHarbor lab F24"};
    if (ioctl(3,UI_SET_EVBIT,EV_KEY)<0 || ioctl(3,UI_SET_KEYBIT,KEY_F24)<0 ||
        ioctl(3,UI_DEV_SETUP,&setup)<0 || ioctl(3,UI_DEV_CREATE)<0) return 10;
    /* Give udev/libinput time to attach on the lab console. Device teardown
     * also occurs on process exit, including watchdog or I/O failure. */
    struct timespec delay = {.tv_sec=2};
    if (clock_nanosleep(CLOCK_MONOTONIC,0,&delay,NULL)) return 10;
    struct input_event frames[4];
    int n = f24_frames(frames);
    size_t bytes = (size_t)n * sizeof *frames;
    if (write(3,frames,bytes) != (ssize_t)bytes) return 10;
    delay = (struct timespec){.tv_nsec=200000000};
    if (clock_nanosleep(CLOCK_MONOTONIC,0,&delay,NULL)) return 10;
    return ioctl(3,UI_DEV_DESTROY)<0 ? 10 : 0;
}
