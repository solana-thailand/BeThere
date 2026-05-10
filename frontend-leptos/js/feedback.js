/**
 * Haptic and audio feedback for the scanner page.
 *
 * Provides vibration patterns and Web Audio beeps so staff get instant
 * confirmation without looking at the screen.
 *
 * - Success: short vibration + high-pitched beep
 * - Warning: double-pulse vibration + medium tone
 * - Error: long vibration + low tone
 *
 * Audio is opt-in: the first call shows a brief overlay asking the user
 * to enable sound. Once accepted, beeps play for all subsequent scans.
 * Vibration works without opt-in on Android (no-op on iOS).
 */

// Audio context is lazy-created on first user interaction to comply with
// browser autoplay policies.
let audioCtx = null;

// Persisted in sessionStorage so it survives page navigation within the session.
function isAudioEnabled() {
  try {
    return sessionStorage.getItem("__bt_audio") === "1";
  } catch {
    return false;
  }
}

function setAudioEnabled(v) {
  try {
    sessionStorage.setItem("__bt_audio", v ? "1" : "0");
  } catch {
    // ignore storage errors
  }
}

/**
 * Enable audio feedback for the current session.
 * Called from the Rust-side toggle button.
 */
export function enableAudio() {
  setAudioEnabled(true);
  // Pre-warm the AudioContext on explicit enable
  getAudioCtx();
}

/**
 * Disable audio feedback for the current session.
 */
export function disableAudio() {
  setAudioEnabled(false);
}

/**
 * Check if audio is currently enabled.
 */
export function isAudioFeedbackEnabled() {
  return isAudioEnabled();
}

/**
 * Play a success feedback: short vibration + ascending beep.
 */
export function feedbackSuccess() {
  vibrate([100]);
  if (isAudioEnabled()) playTone(880, 0.12, "sine");
}

/**
 * Play a warning feedback: double-pulse vibration + medium tone.
 */
export function feedbackWarning() {
  vibrate([50, 50, 50]);
  if (isAudioEnabled()) playTone(660, 0.15, "triangle");
}

/**
 * Play an error feedback: long vibration + low tone.
 */
export function feedbackError() {
  vibrate([200]);
  if (isAudioEnabled()) playTone(330, 0.2, "sawtooth");
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

function vibrate(pattern) {
  try {
    if (navigator.vibrate) {
      navigator.vibrate(pattern);
    }
  } catch {
    // Not supported — silent no-op (e.g. iOS Safari)
  }
}

function getAudioCtx() {
  if (!audioCtx) {
    try {
      audioCtx = new (window.AudioContext || window.webkitAudioContext)();
    } catch {
      return null;
    }
  }
  // Resume if suspended (autoplay policy)
  if (audioCtx.state === "suspended") {
    audioCtx.resume();
  }
  return audioCtx;
}

function playTone(freq, duration, type) {
  const ctx = getAudioCtx();
  if (!ctx) return;

  try {
    const osc = ctx.createOscillator();
    const gain = ctx.createGain();

    osc.type = type;
    osc.frequency.setValueAtTime(freq, ctx.currentTime);

    // Short fade-in / fade-out to avoid clicks
    gain.gain.setValueAtTime(0, ctx.currentTime);
    gain.gain.linearRampToValueAtTime(0.3, ctx.currentTime + 0.01);
    gain.gain.linearRampToValueAtTime(0, ctx.currentTime + duration);

    osc.connect(gain);
    gain.connect(ctx.destination);

    osc.start(ctx.currentTime);
    osc.stop(ctx.currentTime + duration + 0.01);
  } catch {
    // AudioContext may be in a bad state — ignore
  }
}
