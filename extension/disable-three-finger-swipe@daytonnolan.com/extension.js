import * as Main from 'resource:///org/gnome/shell/ui/main.js';

// Each SwipeTracker's touchpad gesture listens on global.stage via
// connectObject(..., this), so it can be detached and reattached cleanly.
function touchpadGestures() {
    const controls = Main.overview._overview?._controls;
    return [
        Main.overview._swipeTracker,
        Main.wm._workspaceAnimation?._swipeTracker,
        controls?._workspacesDisplay?._swipeTracker,
        controls?._appDisplay?._swipeTracker,
    ].map(t => t?._touchpadGesture).filter(Boolean);
}

export default class DisableThreeFingerSwipe {
    enable() {
        this._gestures = touchpadGestures();
        for (const g of this._gestures)
            global.stage.disconnectObject(g);
    }

    disable() {
        for (const g of this._gestures ?? []) {
            global.stage.connectObject(
                'captured-event::touchpad', g._handleEvent.bind(g), g);
        }
        this._gestures = null;
    }
}
