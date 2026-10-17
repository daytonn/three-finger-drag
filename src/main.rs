//! Three-finger drag for Linux touchpads (works on Wayland and X11).
//!
//! Reads the touchpad's raw multitouch events. While three fingers are down and
//! moving, a virtual mouse holds the left button and moves the pointer with the
//! fingers. Lifting the fingers releases the button.
//!
//! libinput treats 3+ finger contact as a gesture (no pointer motion), so there is
//! no double movement. GNOME's own 3-finger swipe must be disabled separately.

use std::io;
use std::os::fd::AsRawFd;
use std::process::ExitCode;
use std::thread::sleep;
use std::time::{Duration, Instant};

use evdev::raw_stream::{self, RawDevice};
use evdev::uinput::VirtualDevice;
use evdev::{
    AbsoluteAxisCode as Abs, AttributeSet, EventType, InputEvent, KeyCode, RelativeAxisCode,
    SynchronizationCode,
};

const MAX_SLOTS: usize = 16;
const FINGER_TOOLS: [(KeyCode, u8); 5] = [
    (KeyCode::BTN_TOOL_FINGER, 1),
    (KeyCode::BTN_TOOL_DOUBLETAP, 2),
    (KeyCode::BTN_TOOL_TRIPLETAP, 3),
    (KeyCode::BTN_TOOL_QUADTAP, 4),
    (KeyCode::BTN_TOOL_QUINTTAP, 5),
];

struct Options {
    speed: f64,
    threshold: f64,
    release_delay: Duration,
}

fn parse_args() -> Result<Options, String> {
    let mut opts = Options { speed: 4.0, threshold: 1.0, release_delay: Duration::ZERO };
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        let (flag, inline) = match arg.split_once('=') {
            Some((f, v)) => (f.to_string(), Some(v.to_string())),
            None => (arg, None),
        };
        if flag == "-h" || flag == "--help" {
            println!(
                "Usage: three-finger-drag [--speed N] [--threshold MM] [--release-delay S]\n\n\
                 --speed N           pointer units per mm of finger travel (default 4.0)\n\
                 --threshold MM      mm of movement before the drag starts, so 3-finger taps still work (default 1.0)\n\
                 --release-delay S   seconds to keep the button held after lifting (default 0)"
            );
            std::process::exit(0);
        }
        let value = inline
            .or_else(|| args.next())
            .ok_or_else(|| format!("{flag} needs a value"))?;
        let num: f64 = value.parse().map_err(|_| format!("invalid number for {flag}: {value}"))?;
        match flag.as_str() {
            "--speed" => opts.speed = num,
            "--threshold" => opts.threshold = num,
            "--release-delay" => opts.release_delay = Duration::from_secs_f64(num.max(0.0)),
            _ => return Err(format!("unknown option: {flag}")),
        }
    }
    Ok(opts)
}

fn find_touchpad() -> Option<RawDevice> {
    raw_stream::enumerate().map(|(_, dev)| dev).find(|dev| {
        dev.supported_absolute_axes().is_some_and(|a| a.contains(Abs::ABS_MT_POSITION_X))
            && dev.supported_keys().is_some_and(|k| k.contains(KeyCode::BTN_TOOL_TRIPLETAP))
    })
}

struct Dragger {
    pad: RawDevice,
    ui: VirtualDevice,
    res: (f64, f64),
    scale: (f64, f64),
    threshold: f64,
    release_delay: Duration,

    slot: usize,
    slots: [Option<(Option<i32>, Option<i32>)>; MAX_SLOTS],
    fingers: u8,
    tracking: bool, // 3 fingers down, waiting for motion threshold
    dragging: bool, // left button is held
    anchor: (f64, f64),
    last: (f64, f64),
    last_ids: u16,
    rem: (f64, f64),
    release_at: Option<Instant>,
}

impl Dragger {
    fn new(pad: RawDevice, opts: &Options) -> io::Result<Self> {
        let mut res = (40.0, 40.0);
        for (axis, info) in pad.get_absinfo()? {
            let r = if info.resolution() > 0 { info.resolution() as f64 } else { 40.0 };
            if axis == Abs::ABS_MT_POSITION_X {
                res.0 = r;
            } else if axis == Abs::ABS_MT_POSITION_Y {
                res.1 = r;
            }
        }

        let mut keys = AttributeSet::<KeyCode>::new();
        keys.insert(KeyCode::BTN_LEFT);
        keys.insert(KeyCode::BTN_RIGHT);
        keys.insert(KeyCode::BTN_MIDDLE);
        let mut rel = AttributeSet::<RelativeAxisCode>::new();
        rel.insert(RelativeAxisCode::REL_X);
        rel.insert(RelativeAxisCode::REL_Y);
        let ui = VirtualDevice::builder()?
            .name("three-finger-drag virtual mouse")
            .with_keys(&keys)?
            .with_relative_axes(&rel)?
            .build()?;

        Ok(Self {
            pad,
            ui,
            res,
            // Convert device units -> mm -> output units
            scale: (opts.speed / res.0, opts.speed / res.1),
            threshold: opts.threshold,
            release_delay: opts.release_delay,
            slot: 0,
            slots: [None; MAX_SLOTS],
            fingers: 0,
            tracking: false,
            dragging: false,
            anchor: (0.0, 0.0),
            last: (0.0, 0.0),
            last_ids: 0,
            rem: (0.0, 0.0),
            release_at: None,
        })
    }

    fn press(&mut self, down: bool) -> io::Result<()> {
        let ev = InputEvent::new(EventType::KEY.0, KeyCode::BTN_LEFT.0, down as i32);
        self.ui.emit(&[ev])?;
        self.dragging = down;
        Ok(())
    }

    /// Centroid and slot bitmask of active contacts, if all have known positions.
    fn contacts(&self) -> Option<((f64, f64), u16)> {
        let (mut sx, mut sy, mut n, mut ids) = (0.0, 0.0, 0.0, 0u16);
        for (i, s) in self.slots.iter().enumerate() {
            if let Some((x, y)) = s {
                sx += (*x)? as f64;
                sy += (*y)? as f64;
                n += 1.0;
                ids |= 1 << i;
            }
        }
        (n > 0.0).then(|| ((sx / n, sy / n), ids))
    }

    fn handle(&mut self, ev: InputEvent) -> io::Result<()> {
        let (code, value) = (ev.code(), ev.value());
        match ev.event_type() {
            EventType::ABSOLUTE => match Abs(code) {
                Abs::ABS_MT_SLOT => self.slot = value as usize,
                Abs::ABS_MT_TRACKING_ID if self.slot < MAX_SLOTS => {
                    self.slots[self.slot] = (value != -1).then_some((None, None));
                }
                Abs::ABS_MT_POSITION_X => {
                    if let Some(Some(p)) = self.slots.get_mut(self.slot) {
                        p.0 = Some(value);
                    }
                }
                Abs::ABS_MT_POSITION_Y => {
                    if let Some(Some(p)) = self.slots.get_mut(self.slot) {
                        p.1 = Some(value);
                    }
                }
                _ => {}
            },
            EventType::KEY => {
                if let Some(&(_, n)) = FINGER_TOOLS.iter().find(|(k, _)| k.0 == code) {
                    if value != 0 {
                        self.fingers = n;
                    } else if self.fingers == n {
                        self.fingers = 0;
                    }
                }
            }
            EventType::SYNCHRONIZATION if code == SynchronizationCode::SYN_REPORT.0 => {
                self.frame()?;
            }
            _ => {}
        }
        Ok(())
    }

    fn frame(&mut self) -> io::Result<()> {
        let contacts = if self.fingers == 3 { self.contacts() } else { None };
        let Some((c, ids)) = contacts else {
            self.tracking = false;
            if self.dragging && self.release_at.is_none() {
                self.release_at = Some(Instant::now() + self.release_delay);
            }
            return Ok(());
        };

        self.release_at = None;
        if !self.tracking {
            self.tracking = true;
            self.anchor = c;
            self.last = c;
            self.last_ids = ids;
            self.rem = (0.0, 0.0);
            return Ok(());
        }
        if ids != self.last_ids {
            // Contact set changed; rebase to avoid a jump
            self.last = c;
            self.last_ids = ids;
            return Ok(());
        }
        if !self.dragging {
            let dx_mm = (c.0 - self.anchor.0) / self.res.0;
            let dy_mm = (c.1 - self.anchor.1) / self.res.1;
            if dx_mm.hypot(dy_mm) < self.threshold {
                return Ok(());
            }
            self.press(true)?;
        }
        self.move_by(c.0 - self.last.0, c.1 - self.last.1)?;
        self.last = c;
        Ok(())
    }

    fn move_by(&mut self, dx: f64, dy: f64) -> io::Result<()> {
        let fx = dx * self.scale.0 + self.rem.0;
        let fy = dy * self.scale.1 + self.rem.1;
        let (ix, iy) = (fx.trunc(), fy.trunc());
        self.rem = (fx - ix, fy - iy);
        let mut events = Vec::with_capacity(2);
        if ix != 0.0 {
            events.push(InputEvent::new(EventType::RELATIVE.0, RelativeAxisCode::REL_X.0, ix as i32));
        }
        if iy != 0.0 {
            events.push(InputEvent::new(EventType::RELATIVE.0, RelativeAxisCode::REL_Y.0, iy as i32));
        }
        if !events.is_empty() {
            self.ui.emit(&events)?;
        }
        Ok(())
    }

    fn tick(&mut self) -> io::Result<()> {
        if self.release_at.is_some_and(|t| Instant::now() >= t) {
            self.release_at = None;
            if self.dragging && !self.tracking {
                self.press(false)?;
            }
        }
        Ok(())
    }

    fn poll(&self) -> io::Result<bool> {
        let timeout = match self.release_at {
            Some(t) => t.saturating_duration_since(Instant::now()).as_millis() as i32,
            None => -1,
        };
        let mut fds = libc::pollfd { fd: self.pad.as_raw_fd(), events: libc::POLLIN, revents: 0 };
        // SAFETY: fds points to one valid pollfd for the duration of the call.
        let n = unsafe { libc::poll(&mut fds, 1, timeout) };
        if n < 0 {
            let err = io::Error::last_os_error();
            return if err.kind() == io::ErrorKind::Interrupted { Ok(false) } else { Err(err) };
        }
        if fds.revents & (libc::POLLERR | libc::POLLHUP | libc::POLLNVAL) != 0 {
            return Err(io::Error::new(io::ErrorKind::NotConnected, "touchpad disconnected"));
        }
        Ok(n > 0)
    }

    fn run(&mut self) -> io::Result<()> {
        loop {
            if self.poll()? {
                let events: Vec<InputEvent> = self.pad.fetch_events()?.collect();
                for ev in events {
                    self.handle(ev)?;
                }
            }
            self.tick()?;
        }
    }
}

impl Drop for Dragger {
    fn drop(&mut self) {
        if self.dragging {
            let _ = self.press(false);
        }
    }
}

fn main() -> ExitCode {
    let opts = match parse_args() {
        Ok(o) => o,
        Err(e) => {
            eprintln!("three-finger-drag: {e}");
            return ExitCode::from(2);
        }
    };

    let pad = loop {
        if let Some(pad) = find_touchpad() {
            break pad;
        }
        eprintln!("three-finger-drag: no multitouch touchpad found, retrying...");
        sleep(Duration::from_secs(5));
    };
    println!("three-finger-drag: using {}", pad.name().unwrap_or("touchpad"));

    // Device removal (e.g. suspend/resume) ends up here; systemd restarts us
    let result = Dragger::new(pad, &opts).and_then(|mut d| d.run());
    if let Err(e) = result {
        eprintln!("three-finger-drag: {e}");
    }
    ExitCode::FAILURE
}
