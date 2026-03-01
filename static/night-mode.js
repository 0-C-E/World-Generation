// ---------------------------------------------------------------------------
// Night mode — auto-schedule + manual override
//
// Default: dark between 18h00 and 06h00, light otherwise.
// The user can override at any time with the button; the override persists
// until the *next* automatic switch (i.e. it resets on the hour boundary).
// ---------------------------------------------------------------------------

let NIGHT_HOUR_START = 18; // 18h00 → night
let NIGHT_HOUR_END = 6;  // 06h00 → day

let OVERRIDE_KEY = 'worldviewer_night_override'; // '0' | '1' | null
let OVERRIDE_TS_KEY = 'worldviewer_night_override_ts';

let tilePaneEl = null;
function getTilePane() {
    if (!tilePaneEl) tilePaneEl = document.querySelector('.leaflet-tile-pane');
    return tilePaneEl;
}

function shouldBeNightNow() {
    let h = new Date().getHours(); // 0-23
    return h >= NIGHT_HOUR_START || h < NIGHT_HOUR_END;
}

// Returns the epoch-ms of the next auto-switch boundary from now.
function nextSwitchTime() {
    let now = new Date();
    let h = now.getHours();
    let next = new Date(now);
    if (h >= NIGHT_HOUR_START) {
        // Currently night → next switch at 06h00 tomorrow.
        next.setDate(next.getDate() + 1);
        next.setHours(NIGHT_HOUR_END, 0, 0, 0);
    } else if (h < NIGHT_HOUR_END) {
        // Currently night (past midnight) → next switch at 06h00 today.
        next.setHours(NIGHT_HOUR_END, 0, 0, 0);
    } else {
        // Currently day → next switch at 18h00 today.
        next.setHours(NIGHT_HOUR_START, 0, 0, 0);
    }
    return next.getTime();
}

function applyNightMode(enabled) {
    let pane = getTilePane();
    if (!pane) return;
    if (enabled) {
        pane.classList.add('night');
        document.getElementById('btn-night').textContent = '🌙';
        document.getElementById('btn-night').title = 'Mode jour (auto: 6h)';
    } else {
        pane.classList.remove('night');
        document.getElementById('btn-night').textContent = '☀️';
        document.getElementById('btn-night').title = 'Mode nuit (auto: 18h)';
    }
}

// Expire a manual override once the next auto-switch boundary is crossed.
function clearExpiredOverride() {
    let ts = parseInt(localStorage.getItem(OVERRIDE_TS_KEY) || '0', 10);
    if (ts && Date.now() >= ts) {
        localStorage.removeItem(OVERRIDE_KEY);
        localStorage.removeItem(OVERRIDE_TS_KEY);
    }
}

function resolveNightMode() {
    clearExpiredOverride();
    let override = localStorage.getItem(OVERRIDE_KEY);
    if (override !== null) return override === '1';
    return shouldBeNightNow();
}

// Apply immediately on load.
let nightMode = resolveNightMode();
setTimeout(function () { applyNightMode(nightMode); }, 0);

// Manual override button.
document.getElementById('btn-night').addEventListener('click', function () {
    nightMode = !nightMode;
    localStorage.setItem(OVERRIDE_KEY, nightMode ? '1' : '0');
    // Override expires at the next automatic boundary.
    localStorage.setItem(OVERRIDE_TS_KEY, String(nextSwitchTime()));
    applyNightMode(nightMode);
});

// Auto-switch: schedule a timer for the next boundary, then repeat.
function scheduleAutoSwitch() {
    let delay = nextSwitchTime() - Date.now();
    setTimeout(function () {
        localStorage.removeItem(OVERRIDE_KEY);
        localStorage.removeItem(OVERRIDE_TS_KEY);
        nightMode = shouldBeNightNow();
        applyNightMode(nightMode);
        scheduleAutoSwitch();
    }, delay);
}

scheduleAutoSwitch();
