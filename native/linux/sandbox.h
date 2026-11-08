#pragma once
#include "scanout.h"
#include <errno.h>
#include <linux/audit.h>
#include <linux/dma-buf.h>
#include <linux/filter.h>
#include <linux/seccomp.h>
#include <sys/prctl.h>
#include <sys/syscall.h>
#include <sys/mman.h>
#if !defined(__x86_64__) || defined(__ILP32__)
#error The DeskHarbor Linux proof worker requires x86_64 LP64.
#endif

#define LOAD(field) BPF_STMT(BPF_LD|BPF_W|BPF_ABS, offsetof(struct seccomp_data, field))
#define MATCH(v,t,f) BPF_JUMP(BPF_JMP|BPF_JEQ|BPF_K, (v), (t), (f))
#define RET(value) BPF_STMT(BPF_RET|BPF_K, (value))
#define DENY RET(SECCOMP_RET_ERRNO|EPERM)
#define PERMIT(v) MATCH(v,0,1), RET(SECCOMP_RET_ALLOW)

/* No open/socket/exec/clone after device selection. Fd 3 is the verified DRM
 * card, fd 1 is a pipe. Only the one newly exported DMA-BUF needs SYNC. */
static inline int lockdown(void) {
    struct sock_filter instructions[] = {
        LOAD(arch), MATCH(AUDIT_ARCH_X86_64,1,0), RET(SECCOMP_RET_KILL_PROCESS),
        LOAD(nr),
        PERMIT(SYS_exit), PERMIT(SYS_exit_group), PERMIT(SYS_brk),
        PERMIT(SYS_munmap), PERMIT(SYS_close), PERMIT(SYS_lseek),
        MATCH(SYS_mmap,0,4), LOAD(args[2]),
            BPF_JUMP(BPF_JMP|BPF_JSET|BPF_K, PROT_EXEC,0,1), DENY, RET(SECCOMP_RET_ALLOW),
        MATCH(SYS_write,0,4), LOAD(args[0]), MATCH(1,1,0), DENY, RET(SECCOMP_RET_ALLOW),
        MATCH(SYS_ioctl,1,0), DENY,
        LOAD(args[1]), PERMIT(DMA_BUF_IOCTL_SYNC),
        LOAD(args[0]), MATCH(3,1,0), DENY,
        LOAD(args[1]),
        PERMIT(DRM_IOCTL_SET_CLIENT_CAP), PERMIT(DRM_IOCTL_MODE_GETCONNECTOR),
        PERMIT(DRM_IOCTL_MODE_GETENCODER), PERMIT(DRM_IOCTL_MODE_GETCRTC),
        PERMIT(DRM_IOCTL_MODE_GETPLANERESOURCES), PERMIT(DRM_IOCTL_MODE_GETPLANE),
        PERMIT(DRM_IOCTL_MODE_OBJ_GETPROPERTIES), PERMIT(DRM_IOCTL_MODE_GETPROPERTY),
        PERMIT(DRM_IOCTL_MODE_GETFB2), PERMIT(DRM_IOCTL_PRIME_HANDLE_TO_FD),
        PERMIT(DRM_IOCTL_GEM_CLOSE), DENY
    };
    struct sock_fprog filter = {
        .len = sizeof instructions / sizeof *instructions, .filter = instructions
    };
    if (prctl(PR_SET_NO_NEW_PRIVS, 1L, 0L, 0L, 0L) < 0) return -1;
    return prctl(PR_SET_SECCOMP, SECCOMP_MODE_FILTER, &filter);
}
#undef LOAD
#undef MATCH
#undef RET
#undef DENY
#undef PERMIT
