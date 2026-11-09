# Linux native texture smoke

This fixture builds an isolated copy of the real Flutter runner, bridge and
texture plugin. It authenticates to an explicitly synthetic, loopback-only
`dh-hostd`, decodes actual VP8, paints native textures and completes eight
create/present/dispose cycles. An external screenshot checks the synthetic green
plane; the fixture never reads the operating-system desktop or clipboard and
never injects input. The capture is of the disposable test X display only.

Build the release Rust library/host/CLI first. Install the normal Flutter Linux
build prerequisites plus Xvfb, scrot and Python Pillow. Run on a test runner:

```sh
DESKHARBOR_SYNTHETIC_DISPLAY=1 xvfb-run -a -s '-screen 0 1024x768x24' \
  /usr/bin/python3 tests/texture_smoke/run.py --output target/texture-smoke
```

The production bundle is never overwritten. The probe runs from its own
temporary project and loads the real packaged `libdh_ffi.so`; no test frame API
is added to that library. Missing pixels, native errors, timeouts and incomplete
cleanup fail the command. Artifacts contain synthetic pixels and fixture logs.
This is a renderer integration check, not a DRM/KMS, Wayland, unattended sign-in,
GPU compatibility, throughput or input-latency qualification.
