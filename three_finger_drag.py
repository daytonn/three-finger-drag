#!/usr/bin/env python3
"""Three-finger drag for Linux touchpads (works on Wayland and X11).

Reads the touchpad's raw multitouch events. While three fingers are down and
moving, a virtual mouse holds the left button and moves the pointer with the
fingers. Lifting the fingers releases the button.

libinput treats 3+ finger contact as a gesture (no pointer motion), so there is
no double movement. GNOME's own 3-finger swipe must be disabled separately.
"""

import argparse
import select
import sys
import time

import evdev
from evdev import ecodes as e

FINGER_TOOLS = {
    e.BTN_TOOL_FINGER: 1,
    e.BTN_TOOL_DOUBLETAP: 2,
    e.BTN_TOOL_TRIPLETAP: 3,
    e.BTN_TOOL_QUADTAP: 4,
    e.BTN_TOOL_QUINTTAP: 5,
}


def find_touchpad():
    for path in evdev.list_devices():
        dev = evdev.InputDevice(path)
        caps = dev.capabilities()
        if (e.ABS_MT_POSITION_X in dict(caps.get(e.EV_ABS, []))
                and e.BTN_TOOL_TRIPLETAP in caps.get(e.EV_KEY, [])):
            return dev
        dev.close()
    return None


class Dragger:
    def __init__(self, touchpad, speed, threshold_mm, release_delay):
        self.pad = touchpad
        abs_info = dict(touchpad.capabilities()[e.EV_ABS])
        res_x = abs_info[e.ABS_MT_POSITION_X].resolution or 40
        res_y = abs_info[e.ABS_MT_POSITION_Y].resolution or 40
        # Convert device units -> mm -> output units
        self.scale_x = speed / res_x
        self.scale_y = speed / res_y
        self.threshold = threshold_mm
        self.res = (res_x, res_y)
        self.release_delay = release_delay

        self.ui = evdev.UInput(
            {e.EV_KEY: [e.BTN_LEFT, e.BTN_RIGHT, e.BTN_MIDDLE],
             e.EV_REL: [e.REL_X, e.REL_Y]},
            name='three-finger-drag virtual mouse')

        self.slot = 0
        self.slots = {}          # slot -> [x, y] for active contacts
        self.fingers = 0
        self.tracking = False    # 3 fingers down, waiting for motion threshold
        self.dragging = False    # left button is held
        self.anchor = None       # centroid where the 3-finger touch began
        self.last = None         # last centroid
        self.last_ids = None
        self.rem = [0.0, 0.0]
        self.release_at = None

    def centroid(self):
        pts = list(self.slots.values())
        return (sum(p[0] for p in pts) / len(pts),
                sum(p[1] for p in pts) / len(pts))

    def press(self, down):
        self.ui.write(e.EV_KEY, e.BTN_LEFT, 1 if down else 0)
        self.ui.syn()
        self.dragging = down

    def handle(self, ev):
        if ev.type == e.EV_ABS:
            if ev.code == e.ABS_MT_SLOT:
                self.slot = ev.value
            elif ev.code == e.ABS_MT_TRACKING_ID:
                if ev.value == -1:
                    self.slots.pop(self.slot, None)
                else:
                    self.slots[self.slot] = [None, None]
            elif ev.code == e.ABS_MT_POSITION_X and self.slot in self.slots:
                self.slots[self.slot][0] = ev.value
            elif ev.code == e.ABS_MT_POSITION_Y and self.slot in self.slots:
                self.slots[self.slot][1] = ev.value
        elif ev.type == e.EV_KEY and ev.code in FINGER_TOOLS:
            if ev.value:
                self.fingers = FINGER_TOOLS[ev.code]
            elif self.fingers == FINGER_TOOLS[ev.code]:
                self.fingers = 0
        elif ev.type == e.EV_SYN and ev.code == e.SYN_REPORT:
            self.frame()

    def frame(self):
        if self.fingers == 3 and all(None not in p for p in self.slots.values()) and self.slots:
            self.release_at = None
            c = self.centroid()
            ids = frozenset(self.slots)
            if not self.tracking:
                self.tracking = True
                self.anchor = self.last = c
                self.last_ids = ids
                self.rem = [0.0, 0.0]
                return
            if ids != self.last_ids:
                # Contact set changed; rebase to avoid a jump
                self.last, self.last_ids = c, ids
                return
            if not self.dragging:
                dx_mm = (c[0] - self.anchor[0]) / self.res[0]
                dy_mm = (c[1] - self.anchor[1]) / self.res[1]
                if (dx_mm ** 2 + dy_mm ** 2) ** 0.5 < self.threshold:
                    return
                self.press(True)
            self.move(c[0] - self.last[0], c[1] - self.last[1])
            self.last = c
        else:
            self.tracking = False
            if self.dragging and self.release_at is None:
                self.release_at = time.monotonic() + self.release_delay

    def move(self, dx, dy):
        fx = dx * self.scale_x + self.rem[0]
        fy = dy * self.scale_y + self.rem[1]
        ix, iy = int(fx), int(fy)
        self.rem = [fx - ix, fy - iy]
        if ix or iy:
            if ix:
                self.ui.write(e.EV_REL, e.REL_X, ix)
            if iy:
                self.ui.write(e.EV_REL, e.REL_Y, iy)
            self.ui.syn()

    def tick(self):
        if self.release_at is not None and time.monotonic() >= self.release_at:
            self.release_at = None
            if self.dragging and not self.tracking:
                self.press(False)

    def run(self):
        print(f'three-finger-drag: using {self.pad.name} ({self.pad.path})', flush=True)
        try:
            while True:
                timeout = None
                if self.release_at is not None:
                    timeout = max(0.0, self.release_at - time.monotonic())
                r, _, _ = select.select([self.pad.fd], [], [], timeout)
                if r:
                    for ev in self.pad.read():
                        self.handle(ev)
                self.tick()
        finally:
            if self.dragging:
                self.press(False)
            self.ui.close()


def main():
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument('--speed', type=float, default=4.0,
                    help='pointer units per mm of finger travel (default 4.0)')
    ap.add_argument('--threshold', type=float, default=1.0,
                    help='mm of movement before the drag starts, so 3-finger taps still work (default 1.0)')
    ap.add_argument('--release-delay', type=float, default=0.0,
                    help='seconds to keep the button held after lifting, letting you reposition fingers (default 0)')
    args = ap.parse_args()

    while True:
        pad = find_touchpad()
        if pad:
            break
        print('three-finger-drag: no multitouch touchpad found, retrying...', file=sys.stderr, flush=True)
        time.sleep(5)

    try:
        Dragger(pad, args.speed, args.threshold, args.release_delay).run()
    except OSError as err:
        # Device removed (e.g. suspend/resume); let systemd restart us
        print(f'three-finger-drag: {err}', file=sys.stderr)
        sys.exit(1)


if __name__ == '__main__':
    main()
