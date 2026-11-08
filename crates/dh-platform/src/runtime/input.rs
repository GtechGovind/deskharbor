//! Platform-owned ordinary desktop input and pure, sink-driven packet accounting.
use crate::report::{Display, ProbeError};

#[derive(Debug, Clone)]
pub enum NativeInput {
    Key {
        usage: u16,
        pressed: bool,
    },
    /// Wheel values use Windows wheel units: +120 is up/right one detent.
    Pointer {
        x: f64,
        y: f64,
        buttons: u32,
        wheel_x: i32,
        wheel_y: i32,
    },
    Text(String),
    ReleaseAll,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Packet {
    Key {
        vk: u16,
        scan: u16,
        flags: u32,
    },
    Mouse {
        x: i32,
        y: i32,
        data: u32,
        flags: u32,
    },
}

#[derive(Clone, Copy)]
pub(super) enum Context {
    Ready,
    LayoutChanged,
    PrincipalLost,
}
impl From<bool> for Context {
    fn from(active: bool) -> Self {
        if active {
            Self::Ready
        } else {
            Self::PrincipalLost
        }
    }
}

fn invalid() -> ProbeError {
    ProbeError::new(
        "invalid_input",
        "Unsupported or invalid ordinary desktop input",
    )
}

// USB page 07 -> Windows Set 1, independent of the host's character layout.
// https://learn.microsoft.com/windows/win32/inputdev/about-keyboard-input
// https://download.microsoft.com/download/1/6/1/161ba512-40e2-4cc9-843a-923143f3456c/translate.pdf
pub(super) fn key_packet(usage: u16, pressed: bool) -> Result<Packet, ProbeError> {
    let scan = match usage {
        4..=0x1d => [
            0x1e, 0x30, 0x2e, 0x20, 0x12, 0x21, 0x22, 0x23, 0x17, 0x24, 0x25, 0x26, 0x32, 0x31,
            0x18, 0x19, 0x10, 0x13, 0x1f, 0x14, 0x16, 0x2f, 0x11, 0x2d, 0x15, 0x2c,
        ][usize::from(usage - 4)],
        0x1e..=0x27 => usage - 0x1c,
        0x28..=0x39 => [
            0x1c, 1, 0xe, 0xf, 0x39, 0xc, 0xd, 0x1a, 0x1b, 0x2b, 0x2b, 0x27, 0x28, 0x29, 0x33,
            0x34, 0x35, 0x3a,
        ][usize::from(usage - 0x28)],
        0x3a..=0x43 => usage + 1,
        0x44 => 0x57,
        0x45 => 0x58,
        0x46 => 0xe037,
        0x47 => 0x46,
        // E1 Pause has no faithful KEYEVENTF_SCANCODE representation; reject it.
        0x49..=0x52 => [
            0xe052, 0xe047, 0xe049, 0xe053, 0xe04f, 0xe051, 0xe04d, 0xe04b, 0xe050, 0xe048,
        ][usize::from(usage - 0x49)],
        0x53..=0x65 => [
            0x45, 0xe035, 0x37, 0x4a, 0x4e, 0xe01c, 0x4f, 0x50, 0x51, 0x4b, 0x4c, 0x4d, 0x47, 0x48,
            0x49, 0x52, 0x53, 0x56, 0xe05d,
        ][usize::from(usage - 0x53)],
        0x67 => 0x59,
        0x68..=0x72 => usage - 4,
        0x73 => 0x76,
        0x85 => 0x7e,
        0x87..=0x8c => [0x73, 0x70, 0x7d, 0x79, 0x7b, 0x5c][usize::from(usage - 0x87)],
        0xe0..=0xe7 => {
            [0x1d, 0x2a, 0x38, 0xe05b, 0xe01d, 0x36, 0xe038, 0xe05c][usize::from(usage - 0xe0)]
        }
        _ => return Err(invalid()),
    };
    Ok(Packet::Key {
        vk: 0,
        scan: scan & 0xff,
        flags: 8 | u32::from(scan & 0xff00 == 0xe000) | if pressed { 0 } else { 2 },
    })
}
fn mouse(flags: u32, data: u32) -> Packet {
    Packet::Mouse {
        x: 0,
        y: 0,
        data,
        flags,
    }
}
fn button(bit: u32, pressed: bool) -> Packet {
    let (down, up, data) = match bit {
        1 => (2, 4, 0),
        2 => (8, 16, 0),
        4 => (32, 64, 0),
        8 => (128, 256, 1),
        16 => (128, 256, 2),
        _ => unreachable!(),
    };
    mouse(if pressed { down } else { up }, data)
}
pub(super) fn packets(
    event: &NativeInput,
    display: &Display,
    displays: &[Display],
    held_buttons: u32,
) -> Result<Vec<Packet>, ProbeError> {
    match event {
        NativeInput::Key { usage, pressed } => Ok(vec![key_packet(*usage, *pressed)?]),
        NativeInput::Text(text) => {
            if text.len() > 8192 || text.contains('\0') {
                return Err(invalid());
            }
            Ok(text
                .encode_utf16()
                .flat_map(|scan| {
                    [
                        Packet::Key {
                            vk: 0,
                            scan,
                            flags: 4,
                        },
                        Packet::Key {
                            vk: 0,
                            scan,
                            flags: 6,
                        },
                    ]
                })
                .collect())
        }
        NativeInput::ReleaseAll => Ok(vec![]),
        NativeInput::Pointer {
            x,
            y,
            buttons,
            wheel_x,
            wheel_y,
        } => {
            if !x.is_finite()
                || !y.is_finite()
                || !(0.0..=1.).contains(x)
                || !(0.0..=1.).contains(y)
                || buttons & !31 != 0
                || wheel_x.unsigned_abs() > 12000
                || wheel_y.unsigned_abs() > 12000
                || displays.is_empty()
            {
                return Err(invalid());
            }
            let valid = |d: &Display| {
                d.width > 0
                    && d.height > 0
                    && i64::from(d.x) + i64::from(d.width) <= i64::from(i32::MAX)
                    && i64::from(d.y) + i64::from(d.height) <= i64::from(i32::MAX)
            };
            if !valid(display)
                || !displays.iter().all(valid)
                || !displays.iter().any(|d| same_display(d, display))
            {
                return Err(invalid());
            }
            let left = displays.iter().map(|d| i64::from(d.x)).min().unwrap();
            let top = displays.iter().map(|d| i64::from(d.y)).min().unwrap();
            let right = displays
                .iter()
                .map(|d| i64::from(d.x) + i64::from(d.width) - 1)
                .max()
                .unwrap();
            let bottom = displays
                .iter()
                .map(|d| i64::from(d.y) + i64::from(d.height) - 1)
                .max()
                .unwrap();
            let normalize = |n: f64, start: i32, size: u32, origin: i64, end: i64| {
                let pixel = i64::from(start) + (n * f64::from(size - 1)).round() as i64;
                if end == origin {
                    0
                } else {
                    ((pixel - origin) as f64 * 65535. / (end - origin) as f64).round() as i32
                }
            };
            let mut result = vec![Packet::Mouse {
                x: normalize(*x, display.x, display.width, left, right),
                y: normalize(*y, display.y, display.height, top, bottom),
                data: 0,
                flags: 0xc001,
            }];
            for bit in [1, 2, 4, 8, 16] {
                if (held_buttons ^ buttons) & bit != 0 {
                    result.push(button(bit, buttons & bit != 0));
                }
            }
            if *wheel_y != 0 {
                result.push(mouse(0x800, *wheel_y as u32));
            }
            if *wheel_x != 0 {
                result.push(mouse(0x1000, *wheel_x as u32));
            }
            Ok(result)
        }
    }
}
pub(super) fn same_display(a: &Display, b: &Display) -> bool {
    a.id == b.id && a.x == b.x && a.y == b.y && a.width == b.width && a.height == b.height
}

#[derive(Default)]
pub(super) struct Held {
    owner: Option<String>,
    keys: std::collections::BTreeSet<(u16, u16, u32)>,
    pub(super) buttons: u32,
}
impl Held {
    pub(super) const fn empty() -> Self {
        Self {
            owner: None,
            keys: std::collections::BTreeSet::new(),
            buttons: 0,
        }
    }
    pub(super) fn forget(&mut self) {
        *self = Self::default();
    }
    fn check(&mut self, owner: &str, active: Context) -> Result<(), ProbeError> {
        if matches!(active, Context::LayoutChanged) {
            return Err(ProbeError::new(
                "stale_layout",
                "Physical monitor layout changed before input delivery",
            ));
        }
        if matches!(active, Context::PrincipalLost) {
            self.forget();
            return Err(ProbeError::new(
                "desktop_changed",
                "Input principal or desktop is no longer active; stale held state discarded",
            ));
        }
        if self.owner.as_deref().is_some_and(|id| id != owner) {
            self.forget();
            return Err(ProbeError::new(
                "permission_denied",
                "Input state belongs to another principal; stale held state discarded",
            ));
        }
        Ok(())
    }
    pub(super) fn deliver<C: Into<Context>>(
        &mut self,
        owner: &str,
        packets: Vec<Packet>,
        mut active: impl FnMut() -> C,
        mut sink: impl FnMut(Packet) -> Result<(), ProbeError>,
    ) -> Result<(), ProbeError> {
        self.check(owner, active().into())?;
        for packet in packets {
            self.check(owner, active().into())?;
            // Unowned key-up packets must not release a locally held key.
            if let Packet::Key { vk, scan, flags } = packet
                && flags & 2 != 0
                && !self.keys.contains(&(vk, scan, flags & !2))
            {
                continue;
            }
            sink(packet)?;
            self.owner = Some(owner.into());
            match packet {
                Packet::Key { vk, scan, flags } => {
                    if flags & 2 == 0 {
                        self.keys.insert((vk, scan, flags));
                    } else {
                        self.keys.remove(&(vk, scan, flags & !2));
                    }
                }
                Packet::Mouse { flags, data, .. } => {
                    for (bit, down, up) in [
                        (1, 2, 4),
                        (2, 8, 16),
                        (4, 32, 64),
                        (8, 128, 256),
                        (16, 128, 256),
                    ] {
                        if bit >= 8 && data & (bit >> 3) == 0 {
                            continue;
                        }
                        if flags & down != 0 {
                            self.buttons |= bit;
                        }
                        if flags & up != 0 {
                            self.buttons &= !bit;
                        }
                    }
                }
            }
        }
        Ok(())
    }
    pub(super) fn release(
        &mut self,
        owner: &str,
        active: impl FnMut() -> bool,
        sink: impl FnMut(Packet) -> Result<(), ProbeError>,
    ) -> Result<(), ProbeError> {
        let mut packets: Vec<_> = self
            .keys
            .iter()
            .map(|&(vk, scan, flags)| Packet::Key {
                vk,
                scan,
                flags: flags | 2,
            })
            .collect();
        for bit in [1, 2, 4, 8, 16] {
            if self.buttons & bit != 0 {
                packets.push(button(bit, false));
            }
        }
        self.deliver(owner, packets, active, sink)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn display(x: i32, y: i32, width: u32, height: u32) -> Display {
        Display {
            id: "fixture".into(),
            name: "fixture".into(),
            x,
            y,
            width,
            height,
            primary: false,
        }
    }
    fn k(scan: u16, flags: u32) -> Packet {
        Packet::Key { vk: 0, scan, flags }
    }
    fn p(x: f64, y: f64, buttons: u32, wheel_x: i32, wheel_y: i32) -> NativeInput {
        NativeInput::Pointer {
            x,
            y,
            buttons,
            wheel_x,
            wheel_y,
        }
    }
    #[test]
    fn physical_us_german_iso_and_e0_keys() {
        // Literal USB/Windows fixtures: same physical Y/Z positions under either host layout.
        for (usage, scan, flags) in [
            (4, 0x1e, 8),
            (0x1c, 0x15, 8),
            (0x1d, 0x2c, 8),
            (0x32, 0x2b, 8),
            (0x64, 0x56, 8),
            (0xe0, 0x1d, 8),
            (0xe1, 0x2a, 8),
            (0xe2, 0x38, 8),
            (0xe3, 0x5b, 9),
            (0xe4, 0x1d, 9),
            (0xe5, 0x36, 8),
            (0xe6, 0x38, 9),
            (0xe7, 0x5c, 9),
            (0x49, 0x52, 9),
            (0x4c, 0x53, 9),
            (0x50, 0x4b, 9),
            (0x53, 0x45, 8),
            (0x54, 0x35, 9),
            (0x58, 0x1c, 9),
            (0x59, 0x4f, 8),
            (0x46, 0x37, 9),
        ] {
            assert_eq!(key_packet(usage, true).unwrap(), k(scan, flags));
            assert_eq!(key_packet(usage, false).unwrap(), k(scan, flags | 2));
        }
        for usage in [0, 1, 3, 0x48, 0x66, 0xffff] {
            assert!(key_packet(usage, true).is_err());
        }
    }
    #[test]
    fn unicode_utf16_has_individual_surrogate_down_up_pairs() {
        let d = display(0, 0, 1920, 1080);
        assert_eq!(
            packets(
                &NativeInput::Text("A😀".into()),
                &d,
                std::slice::from_ref(&d),
                0
            )
            .unwrap(),
            vec![
                k(0x41, 4),
                k(0x41, 6),
                k(0xd83d, 4),
                k(0xd83d, 6),
                k(0xde00, 4),
                k(0xde00, 6)
            ]
        );
        assert!(
            packets(
                &NativeInput::Text("x\0".into()),
                &d,
                std::slice::from_ref(&d),
                0
            )
            .is_err()
        );
        assert!(
            packets(
                &NativeInput::Text("x".repeat(8193)),
                &d,
                std::slice::from_ref(&d),
                0
            )
            .is_err()
        );
    }
    #[test]
    fn physical_negative_origin_and_mixed_monitor_corners() {
        let left = display(-1920, -200, 1920, 1080);
        let right = display(0, 0, 2560, 1440);
        let all = [left.clone(), right.clone()];
        assert_eq!(
            packets(&p(0., 0., 0, 0, 0), &left, &all, 0).unwrap(),
            vec![Packet::Mouse {
                x: 0,
                y: 0,
                data: 0,
                flags: 0xc001
            }]
        );
        assert_eq!(
            packets(&p(1., 1., 0, 0, 0), &right, &all, 0).unwrap(),
            vec![Packet::Mouse {
                x: 65535,
                y: 65535,
                data: 0,
                flags: 0xc001
            }]
        );
        // Physical pixel (0,0), relative to virtual (-1920,-200)..(2559,1439).
        assert_eq!(
            packets(&p(0., 0., 0, 0, 0), &right, &all, 0).unwrap(),
            vec![Packet::Mouse {
                x: 28093,
                y: 7997,
                data: 0,
                flags: 0xc001
            }]
        );
        for event in [
            p(f64::NAN, 0., 0, 0, 0),
            p(0., 1.1, 0, 0, 0),
            p(0., 0., 32, 0, 0),
            p(0., 0., 0, i32::MIN, 0),
        ] {
            assert!(packets(&event, &right, &all, 0).is_err());
        }
        assert!(packets(&p(0., 0., 0, 0, 0), &display(0, 0, 0, 1), &all, 0).is_err());
    }
    #[test]
    fn buttons_are_transitions_and_wheels_are_separate_signed_packets() {
        let d = display(0, 0, 2, 2);
        assert_eq!(
            packets(&p(0., 0., 17, -120, 240), &d, std::slice::from_ref(&d), 1).unwrap(),
            vec![
                Packet::Mouse {
                    x: 0,
                    y: 0,
                    data: 0,
                    flags: 0xc001
                },
                Packet::Mouse {
                    x: 0,
                    y: 0,
                    data: 2,
                    flags: 0x80
                },
                Packet::Mouse {
                    x: 0,
                    y: 0,
                    data: 240,
                    flags: 0x800
                },
                Packet::Mouse {
                    x: 0,
                    y: 0,
                    data: (-120i32) as u32,
                    flags: 0x1000
                }
            ]
        );
    }
    #[test]
    fn partial_delivery_cleanup_releases_only_successful_remote_packets_once() {
        let mut held = Held::default();
        let mut sent = vec![];
        let result = held.deliver(
            "owner",
            vec![k(0x1d, 8), k(0x1e, 8)],
            || true,
            |packet| {
                if sent.len() == 1 {
                    return Err(invalid());
                }
                sent.push(packet);
                Ok(())
            },
        );
        assert!(result.is_err());
        held.release(
            "owner",
            || true,
            |packet| {
                sent.push(packet);
                Ok(())
            },
        )
        .unwrap();
        held.release(
            "owner",
            || true,
            |packet| {
                sent.push(packet);
                Ok(())
            },
        )
        .unwrap();
        assert_eq!(sent, vec![k(0x1d, 8), k(0x1d, 10)]);
    }
    #[test]
    fn loss_of_principal_discards_cleanup_permanently() {
        let mut held = Held::default();
        held.deliver("owner", vec![k(0x38, 9)], || true, |_| Ok(()))
            .unwrap();
        assert!(
            held.release("owner", || false, |_| panic!("must not inject elsewhere"))
                .is_err()
        );
        held.release(
            "owner",
            || true,
            |_| panic!("must not replay stale cleanup"),
        )
        .unwrap();
    }
    #[test]
    fn failed_release_retries_only_unsent_keys_buttons_and_unicode() {
        let mut held = Held::default();
        held.deliver(
            "owner",
            vec![
                k(0xd83d, 4),
                Packet::Mouse {
                    x: 0,
                    y: 0,
                    data: 0,
                    flags: 2,
                },
            ],
            || true,
            |_| Ok(()),
        )
        .unwrap();
        assert!(held.release("owner", || true, |_| Err(invalid())).is_err());
        let mut sent = vec![];
        held.release(
            "owner",
            || true,
            |packet| {
                sent.push(packet);
                Ok(())
            },
        )
        .unwrap();
        assert_eq!(
            sent,
            vec![
                k(0xd83d, 6),
                Packet::Mouse {
                    x: 0,
                    y: 0,
                    data: 0,
                    flags: 4
                }
            ]
        );
    }
    #[test]
    fn new_active_principal_cannot_replay_old_state_or_stay_blocked_by_it() {
        let mut held = Held::default();
        held.deliver("old", vec![k(0x1d, 8)], || true, |_| Ok(()))
            .unwrap();
        assert!(
            held.deliver(
                "new",
                vec![k(0x1e, 8)],
                || true,
                |_| panic!("old owner still held")
            )
            .is_err()
        );
        held.deliver("new", vec![k(0x1e, 8)], || true, |_| Ok(()))
            .unwrap();
        let mut sent = vec![];
        held.release(
            "new",
            || true,
            |p| {
                sent.push(p);
                Ok(())
            },
        )
        .unwrap();
        assert_eq!(sent, vec![k(0x1e, 10)]);
    }
    #[test]
    fn loss_between_packets_never_sends_an_up_into_another_desktop() {
        let mut held = Held::default();
        let mut checks = 0;
        let mut sent = vec![];
        assert!(
            held.deliver(
                "owner",
                vec![k(0x1d, 8), k(0x1e, 8)],
                || {
                    checks += 1;
                    checks <= 2
                },
                |p| {
                    sent.push(p);
                    Ok(())
                }
            )
            .is_err()
        );
        held.release(
            "owner",
            || true,
            |_| panic!("lost desktop must discard held state"),
        )
        .unwrap();
        assert_eq!(sent, vec![k(0x1d, 8)]);
    }
    #[test]
    fn repeat_and_alias_keys_release_once_and_unowned_ups_are_suppressed() {
        let mut held = Held::default();
        let mut sent = vec![];
        held.deliver(
            "owner",
            vec![k(0x2b, 10), k(0x2b, 8), k(0x2b, 8)],
            || true,
            |p| {
                sent.push(p);
                Ok(())
            },
        )
        .unwrap();
        held.release(
            "owner",
            || true,
            |p| {
                sent.push(p);
                Ok(())
            },
        )
        .unwrap();
        held.release(
            "owner",
            || true,
            |p| {
                sent.push(p);
                Ok(())
            },
        )
        .unwrap();
        assert_eq!(sent, vec![k(0x2b, 8), k(0x2b, 8), k(0x2b, 10)]);
    }
    #[test]
    fn all_button_cleanup_uses_only_recorded_successes() {
        let mut held = Held::default();
        let mut count = 0;
        assert!(
            held.deliver(
                "owner",
                vec![
                    mouse(2, 0),
                    mouse(8, 0),
                    mouse(32, 0),
                    mouse(128, 1),
                    mouse(128, 2)
                ],
                || true,
                |_| {
                    count += 1;
                    if count == 5 { Err(invalid()) } else { Ok(()) }
                }
            )
            .is_err()
        );
        let mut sent = vec![];
        held.release(
            "owner",
            || true,
            |p| {
                sent.push(p);
                Ok(())
            },
        )
        .unwrap();
        assert_eq!(
            sent,
            vec![mouse(4, 0), mouse(16, 0), mouse(64, 0), mouse(256, 1)]
        );
    }
    #[test]
    fn partially_successful_release_does_not_repeat_the_successful_up() {
        let mut held = Held::default();
        held.deliver(
            "owner",
            vec![k(0x1d, 8), k(0x38, 8), mouse(2, 0)],
            || true,
            |_| Ok(()),
        )
        .unwrap();
        let mut sent = vec![];
        assert!(
            held.release(
                "owner",
                || true,
                |p| {
                    if sent.len() == 1 {
                        return Err(invalid());
                    }
                    sent.push(p);
                    Ok(())
                }
            )
            .is_err()
        );
        held.release(
            "owner",
            || true,
            |p| {
                sent.push(p);
                Ok(())
            },
        )
        .unwrap();
        assert_eq!(sent, vec![k(0x1d, 10), k(0x38, 10), mouse(4, 0)]);
    }
    #[test]
    fn singleton_pixel_and_stale_or_overflowing_monitor_bounds_are_explicit() {
        let d = display(-1, -1, 1, 1);
        assert_eq!(
            packets(&p(1., 1., 0, 0, 0), &d, std::slice::from_ref(&d), 0).unwrap(),
            vec![Packet::Mouse {
                x: 0,
                y: 0,
                data: 0,
                flags: 0xc001
            }]
        );
        assert!(
            packets(
                &p(0., 0., 0, 0, 0),
                &display(-2, -1, 1, 1),
                std::slice::from_ref(&d),
                0
            )
            .is_err()
        );
        let over = display(i32::MAX, 0, 2, 1);
        assert!(packets(&p(0., 0., 0, 0, 0), &over, std::slice::from_ref(&over), 0).is_err());
    }
    #[test]
    fn layout_change_preserves_owned_keys_for_same_principal_cleanup() {
        let mut held = Held::default();
        let mut checks = 0;
        let mut sent = vec![];
        assert!(
            held.deliver(
                "owner",
                vec![k(0x1d, 8), k(0x1e, 8)],
                || {
                    checks += 1;
                    if checks <= 2 {
                        Context::Ready
                    } else {
                        Context::LayoutChanged
                    }
                },
                |p| {
                    sent.push(p);
                    Ok(())
                }
            )
            .is_err()
        );
        held.release(
            "owner",
            || true,
            |p| {
                sent.push(p);
                Ok(())
            },
        )
        .unwrap();
        assert_eq!(sent, vec![k(0x1d, 8), k(0x1d, 10)]);
    }
}
