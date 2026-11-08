#define _GNU_SOURCE
#include <errno.h>
#include <fcntl.h>
#include <linux/capability.h>
#include <linux/dma-buf.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/auxv.h>
#include <sys/ioctl.h>
#include <sys/mman.h>
#include <sys/stat.h>
#include <sys/sysmacros.h>
#include <sys/syscall.h>
#include <unistd.h>
#include "sandbox.h"

#define CARD 3
#define MAX_OBJECTS 256
#define PTR(p) ((uint64_t)(uintptr_t)(p))

static int failure(void) {
    if (errno == EACCES || errno == EPERM) return PERMISSION;
    if (errno == ENOENT || errno == ENODEV) return NO_SCANOUT;
    return UNAVAILABLE;
}
static int request(unsigned long operation, void *data) {
    int rc;
    do { rc = ioctl(CARD, operation, data); } while (rc < 0 && errno == EINTR);
    return rc == 0 ? OK : failure();
}
static int decimal(const char *s, uint32_t *n) {
    if (!*s || (*s == '0' && s[1])) return 0;
    uint64_t value = 0;
    while (*s) {
        if (*s < '0' || *s > '9') return 0;
        value = value * 10 + (unsigned)(*s++ - '0');
        if (value > UINT32_MAX) return 0;
    }
    *n = (uint32_t)value;
    return 1;
}

/* File capabilities/setuid on a user-invocable probe are unsupported. A
 * future isolated service may launch a non-root dedicated uid with only
 * CAP_SYS_ADMIN (required by GETFB2). Never acquire or install privilege. */
static int safe_identity(void) {
    struct __user_cap_header_struct h = {.version=_LINUX_CAPABILITY_VERSION_3};
    struct __user_cap_data_struct caps[2] = {0};
    if (!geteuid() || getuid() != geteuid() || getgid() != getegid() ||
        getauxval(AT_SECURE) || syscall(SYS_capget, &h, caps) < 0) return 0;
    return !((caps[0].permitted | caps[0].effective | caps[0].inheritable) & ~(1u << CAP_SYS_ADMIN))
        && !(caps[1].permitted | caps[1].effective | caps[1].inheritable);
}
static int open_card(uint32_t minor_id) {
    char path[32];
    snprintf(path, sizeof path, "/dev/dri/card%u", minor_id);
    int fd = open(path, O_RDONLY | O_NOFOLLOW | O_CLOEXEC | O_NONBLOCK);
    if (fd < 0) return failure();
    struct stat metadata;
    if (fstat(fd, &metadata) || !S_ISCHR(metadata.st_mode) ||
        major(metadata.st_rdev) != 226 || minor(metadata.st_rdev) != minor_id)
        return BAD_REQUEST;
    if (fd != CARD && dup3(fd, CARD, O_CLOEXEC) < 0) return UNAVAILABLE;
    if (syscall(SYS_close_range, 4u, ~0u, 0u) < 0) return SANDBOX;
    return OK;
}
static int mode(uint32_t connector, struct state *s) {
    struct drm_mode_modeinfo one_mode;
    /* count_modes > 0 prevents a forced hardware reprobe. */
    struct drm_mode_get_connector c = {.connector_id=connector,.count_modes=1,.modes_ptr=PTR(&one_mode)};
    int rc = request(DRM_IOCTL_MODE_GETCONNECTOR, &c);
    if (rc) return rc;
    if (c.connection != 1 || !c.encoder_id) return NO_SCANOUT;
    struct drm_mode_get_encoder e = {.encoder_id=c.encoder_id};
    if ((rc = request(DRM_IOCTL_MODE_GETENCODER, &e))) return rc;
    if (!e.crtc_id) return NO_SCANOUT;
    struct drm_mode_crtc crtc = {.crtc_id=e.crtc_id};
    if ((rc = request(DRM_IOCTL_MODE_GETCRTC, &crtc))) return rc;
    if (!crtc.mode_valid) return NO_SCANOUT;
    s->crtc = e.crtc_id;
    s->width = crtc.mode.hdisplay;
    s->height = crtc.mode.vdisplay;
    return OK;
}
struct properties {
    uint32_t ids[MAX_OBJECTS], count;
    uint64_t values[MAX_OBJECTS];
};
static int properties(uint32_t id, uint32_t type, struct properties *p) {
    struct drm_mode_obj_get_properties q = {.obj_id=id,.obj_type=type,
        .count_props=MAX_OBJECTS,.props_ptr=PTR(p->ids),.prop_values_ptr=PTR(p->values)};
    int rc = request(DRM_IOCTL_MODE_OBJ_GETPROPERTIES, &q);
    if (rc) return rc;
    if (q.count_props > MAX_OBJECTS) return UNAVAILABLE;
    p->count = q.count_props;
    return OK;
}
static int read_name(uint32_t id, char name[DRM_PROP_NAME_LEN]) {
    struct drm_mode_get_property prop = {.prop_id=id};
    int rc = request(DRM_IOCTL_MODE_GETPROPERTY, &prop);
    if (rc) return rc;
    memcpy(name, prop.name, DRM_PROP_NAME_LEN);
    name[DRM_PROP_NAME_LEN - 1] = 0;
    return OK;
}
static int screen_effects(uint32_t id, uint32_t type) {
    struct properties p;
    int rc = properties(id, type, &p);
    if (rc) return rc;
    for (uint32_t i = 0; i < p.count; i++) {
        char name[DRM_PROP_NAME_LEN];
        if ((rc = read_name(p.ids[i], name))) return rc;
        if (unsupported_screen_effect(name, p.values[i])) return COMPOSITION;
    }
    return OK;
}
static int plane_state(uint32_t id, struct state *s) {
    struct properties p;
    int rc = properties(id, DRM_MODE_OBJECT_PLANE, &p);
    if (rc) return rc;
    s->rotation = 1; s->alpha = 65535;
    struct {const char *name; uint64_t *dest;} fields[] = {
        {"SRC_X",&s->src_x},{"SRC_Y",&s->src_y},{"SRC_W",&s->src_w},{"SRC_H",&s->src_h},
        {"CRTC_X",&s->x},{"CRTC_Y",&s->y},{"CRTC_W",&s->w},{"CRTC_H",&s->h},{"type",&s->type},
        {"rotation",&s->rotation},{"alpha",&s->alpha}
    };
    unsigned seen = 0;
    for (uint32_t i = 0; i < p.count; i++) {
        char name[DRM_PROP_NAME_LEN];
        if ((rc = read_name(p.ids[i], name))) return rc;
        if (!strcmp(name, "FB_ID") && p.values[i] != s->fb) return CHANGED;
        if (!strcmp(name, "CRTC_ID") && p.values[i] != s->crtc) return CHANGED;
        if (!strcmp(name, "COLOR_PIPELINE") && p.values[i]) return COMPOSITION;
        for (unsigned j = 0; j < sizeof fields / sizeof fields[0]; ++j) {
            if (!strcmp(name, fields[j].name)) {
                *fields[j].dest = p.values[i];
                seen |= 1u << j;
            }
        }
    }
    return (seen & 0x1ff) == 0x1ff ? OK : GEOMETRY;
}
static int snapshot(uint32_t connector, struct state *s) {
    memset(s, 0, sizeof *s);
    int rc = mode(connector, s);
    if (rc) return rc;
    if ((rc = screen_effects(s->crtc, DRM_MODE_OBJECT_CRTC))) return rc;
    if ((rc = screen_effects(connector, DRM_MODE_OBJECT_CONNECTOR))) return rc;
    uint32_t ids[MAX_OBJECTS];
    struct drm_mode_get_plane_res planes = {.plane_id_ptr=PTR(ids),.count_planes=MAX_OBJECTS};
    if ((rc = request(DRM_IOCTL_MODE_GETPLANERESOURCES, &planes))) return rc;
    if (planes.count_planes > MAX_OBJECTS) return UNAVAILABLE;
    for (uint32_t i = 0; i < planes.count_planes; i++) {
        struct drm_mode_get_plane plane = {.plane_id=ids[i]};
        if ((rc = request(DRM_IOCTL_MODE_GETPLANE, &plane))) return rc;
        if (plane.crtc_id != s->crtc || !plane.fb_id) continue;
        if (++s->planes != 1) return COMPOSITION;
        s->fb = plane.fb_id;
        if ((rc = plane_state(ids[i], s))) return rc;
    }
    return eligible(s);
}
static int send_all(const void *data, size_t length) {
    const uint8_t *p = data;
    while (length) {
        ssize_t n = write(STDOUT_FILENO, p, length);
        if (n < 0 && errno == EINTR) continue;
        if (n <= 0) return UNAVAILABLE;
        p += n; length -= (size_t)n;
    }
    return OK;
}
static int sync_dma(int fd, uint64_t flags) {
    struct dma_buf_sync sync = {.flags=flags};
    int rc;
    do { rc = ioctl(fd, DMA_BUF_IOCTL_SYNC, &sync); } while (rc < 0 && (errno == EINTR || errno == EAGAIN));
    return rc < 0 ? failure() : OK;
}
static int capture(uint32_t connector, struct state *before) {
    int rc = snapshot(connector, before);
    if (rc) return rc;
    struct drm_mode_fb_cmd2 fb = {.fb_id=before->fb};
    if ((rc = request(DRM_IOCTL_MODE_GETFB2, &fb))) return rc;
    size_t bytes;
    if ((rc = extent(&fb, before, &bytes))) return rc;
    if (!fb.handles[0]) return PERMISSION;
    struct drm_prime_handle prime = {.handle=fb.handles[0],.flags=DRM_CLOEXEC,.fd=-1};
    if ((rc = request(DRM_IOCTL_PRIME_HANDLE_TO_FD, &prime))) return rc;
    /* Process exit releases all GEM handles and exports on every error path. */
    struct drm_gem_close gem = {.handle=fb.handles[0]};
    if ((rc = request(DRM_IOCTL_GEM_CLOSE, &gem))) return rc;
    off_t backing = lseek(prime.fd, 0, SEEK_END);
    if (backing < 0 || (uint64_t)backing < bytes) return INVALID_BUFFER;
    uint8_t *mapping = mmap(NULL, bytes, PROT_READ, MAP_SHARED, prime.fd, 0);
    if (mapping == MAP_FAILED) return failure();
    if ((rc = sync_dma(prime.fd, DMA_BUF_SYNC_START | DMA_BUF_SYNC_READ))) return rc;
    size_t length = (size_t)fb.width * fb.height * 4;
    uint8_t *pixels = malloc(length);
    if (!pixels) return INVALID_BUFFER;
    unpack(&fb, mapping, pixels);
    if ((rc = sync_dma(prime.fd, DMA_BUF_SYNC_END | DMA_BUF_SYNC_READ))) return rc;
    struct state after;
    if ((rc = snapshot(connector, &after))) return rc;
    if (memcmp(before, &after, sizeof after)) return CHANGED;
    uint32_t header[] = {0x31464844, fb.width, fb.height};
    if ((rc = send_all(header, sizeof header))) return rc;
    return send_all(pixels, length);
}
int main(int argc, char **argv) {
    uint32_t card, connector;
    if (argc != 4 || (strcmp(argv[1],"mode") && strcmp(argv[1],"capture")) ||
        !decimal(argv[2], &card) || card > 63 || !decimal(argv[3], &connector) || !connector)
        return BAD_REQUEST;
    struct stat output;
    if (!safe_identity() || fstat(1, &output) || !S_ISFIFO(output.st_mode)) return SANDBOX;
    int rc = open_card(card);
    if (rc) return rc;
    if (prctl(PR_SET_DUMPABLE, 0L) < 0) return SANDBOX;
    close(0); close(2);
    alarm(10);
    if (lockdown()) return SANDBOX;
    struct state state = {0};
    if (!strcmp(argv[1], "mode")) {
        if ((rc = mode(connector, &state))) return rc;
        uint32_t header[] = {0x31464844, state.width, state.height};
        return send_all(header, sizeof header);
    }
    struct drm_set_client_cap cap = {.capability=DRM_CLIENT_CAP_ATOMIC,.value=1};
    if ((rc = request(DRM_IOCTL_SET_CLIENT_CAP, &cap))) return rc;
    return capture(connector, &state);
}
