#define _GNU_SOURCE
#include <sys/socket.h>
#include <sys/ioctl.h>
#include <fcntl.h>
#include <unistd.h>
#include <errno.h>
#include "sandbox.h"
int main(void) {
    if (lockdown() != 0) return 1;
    if (socket(AF_INET, SOCK_STREAM, 0) != -1 || errno != EPERM) return 2;
    if (open("/etc/hostname", O_RDONLY) != -1 || errno != EPERM) return 3;
    if (ioctl(3, 0xdead, 0) != -1 || errno != EPERM) return 4;
    if (write(2, "", 0) != -1 || errno != EPERM) return 5;
    return 0;
}
