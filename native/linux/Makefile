CC ?= cc
BUILD ?= build
CFLAGS ?= -O2 -Wall -Wextra -Wpedantic -Werror -std=c17 -D_FORTIFY_SOURCE=2 -fstack-protector-strong
LDFLAGS ?= -Wl,-z,relro,-z,now,-z,noexecstack
.PHONY: all test
all: $(BUILD)/dh-drm-worker $(BUILD)/dh-uinput-tap
$(BUILD):
	mkdir -p "$@"
$(BUILD)/dh-drm-worker: worker.c sandbox.h scanout.h | $(BUILD)
	$(CC) $(CFLAGS) worker.c -o "$@" $(LDFLAGS)
$(BUILD)/dh-uinput-tap: input_probe.c input_events.h | $(BUILD)
	$(CC) $(CFLAGS) input_probe.c -o "$@" $(LDFLAGS)
$(BUILD)/test-input: test_input.c input_events.h | $(BUILD)
	$(CC) $(CFLAGS) test_input.c -o "$@" $(LDFLAGS)
$(BUILD)/test-scanout: test_scanout.c scanout.h | $(BUILD)
	$(CC) $(CFLAGS) test_scanout.c -o "$@" $(LDFLAGS)
$(BUILD)/test-sandbox: test_sandbox.c sandbox.h scanout.h | $(BUILD)
	$(CC) $(CFLAGS) test_sandbox.c -o "$@" $(LDFLAGS)
test: all $(BUILD)/test-scanout $(BUILD)/test-sandbox $(BUILD)/test-input
	"$(BUILD)/test-scanout"
	"$(BUILD)/test-sandbox"
	"$(BUILD)/test-input"
