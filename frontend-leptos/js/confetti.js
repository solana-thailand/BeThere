/**
 * Confetti animation module.
 *
 * Launches a burst of festive confetti particles that fall from the
 * top of the viewport with random horizontal spread, rotation, and
 * slight wind. Pure JS — no dependencies.
 *
 * Imported via `#[wasm_bindgen(module = "/js/confetti.js")]` in Rust.
 */

/** Festive color palette for confetti particles. */
var COLORS = ["#22c55e", "#8b5cf6", "#3b82f6", "#eab308", "#ec4899", "#f97316"];

/** Number of particles to spawn per launch. */
var PARTICLE_COUNT = 80;

/** Minimum animation duration in seconds. */
var DURATION_MIN = 1.5;

/** Maximum animation duration in seconds. */
var DURATION_MAX = 3.0;

/** Minimum particle size in pixels. */
var SIZE_MIN = 6;

/** Maximum particle size in pixels. */
var SIZE_MAX = 12;

/** Minimum opacity. */
var OPACITY_MIN = 0.7;

/** Maximum opacity. */
var OPACITY_MAX = 1.0;

/** How long (ms) before auto-cleanup removes the container. */
var CLEANUP_DELAY_MS = 3500;

/** Unique keyframe name to avoid collisions. */
var KEYFRAME_NAME = "confetti_fall_" + Date.now();

/**
 * Returns a random float in [min, max).
 *
 * @param {number} min - Lower bound (inclusive).
 * @param {number} max - Upper bound (exclusive).
 * @returns {number}
 */
function rand_range(min, max) {
  return Math.random() * (max - min) + min;
}

/**
 * Picks a random element from an array.
 *
 * @param {any[]} arr
 * @returns {any}
 */
function rand_pick(arr) {
  return arr[Math.floor(Math.random() * arr.length)];
}

/**
 * Injects the confetti keyframe animation into the document <head>.
 *
 * The animation moves particles from above the viewport down with
 * a gentle horizontal drift to simulate wind.
 */
function inject_keyframes() {
  var style = document.createElement("style");
  style.setAttribute("data-confetti", "keyframes");
  style.textContent =
    "@keyframes " + KEYFRAME_NAME + " {" +
    "  0% {" +
    "    transform: translateY(0) translateX(0) rotate(0deg);" +
    "    opacity: 1;" +
    "  }" +
    "  25% {" +
    "    transform: translateY(25vh) translateX(20px) rotate(180deg);" +
    "    opacity: 0.95;" +
    "  }" +
    "  50% {" +
    "    transform: translateY(50vh) translateX(-15px) rotate(360deg);" +
    "    opacity: 0.85;" +
    "  }" +
    "  75% {" +
    "    transform: translateY(75vh) translateX(25px) rotate(540deg);" +
    "    opacity: 0.6;" +
    "  }" +
    "  100% {" +
    "    transform: translateY(105vh) translateX(10px) rotate(720deg);" +
    "    opacity: 0;" +
    "  }" +
    "}";
  document.head.appendChild(style);
}

/**
 * Creates a single confetti particle DOM element.
 *
 * Each particle is a small rectangle or circle with a random color,
 * size, opacity, horizontal offset, and animation duration.
 *
 * @param {HTMLElement} container - Parent container to append into.
 */
function spawn_particle(container) {
  var el = document.createElement("div");

  var size = rand_range(SIZE_MIN, SIZE_MAX);
  var is_circle = Math.random() > 0.5;
  var color = rand_pick(COLORS);
  var opacity = rand_range(OPACITY_MIN, OPACITY_MAX);
  var duration = rand_range(DURATION_MIN, DURATION_MAX);
  var left_pct = rand_range(0, 100);
  var delay = rand_range(0, 0.4);

  // Slight wind variation per particle
  var wind_x = rand_range(-40, 40);

  el.style.position = "absolute";
  el.style.top = "-" + size + "px";
  el.style.left = left_pct + "%";
  el.style.width = size + "px";
  el.style.height = (is_circle ? size : size * 0.6) + "px";
  el.style.backgroundColor = color;
  el.style.opacity = String(opacity);
  el.style.borderRadius = is_circle ? "50%" : "2px";
  el.style.animation = KEYFRAME_NAME + " " + duration + "s ease-in " + delay + "s forwards";
  el.style.transform = "translateX(" + wind_x + "px)";
  el.style.willChange = "transform, opacity";

  container.appendChild(el);
}

/**
 * Launch a burst of confetti particles across the full viewport.
 *
 * Creates a fixed overlay container, spawns ~80 particles with
 * randomized properties, then auto-cleans everything after 3.5 s.
 * Safe to call multiple times — each launch gets its own container.
 */
export function launchConfetti() {
  // Inject keyframes once (idempotent — extra <style> tags are harmless
  // but we avoid duplicates on repeated calls)
  if (!document.querySelector('style[data-confetti="keyframes"]')) {
    inject_keyframes();
  }

  // Create a fresh container for this burst
  var container = document.createElement("div");
  container.style.position = "fixed";
  container.style.top = "0";
  container.style.left = "0";
  container.style.width = "100%";
  container.style.height = "100%";
  container.style.pointerEvents = "none";
  container.style.zIndex = "9999";
  container.style.overflow = "hidden";
  container.setAttribute("aria-hidden", "true");

  document.body.appendChild(container);

  // Spawn particles
  for (var i = 0; i < PARTICLE_COUNT; i++) {
    spawn_particle(container);
  }

  // Auto-cleanup after animations finish
  setTimeout(function () {
    if (container.parentNode) {
      container.parentNode.removeChild(container);
    }
  }, CLEANUP_DELAY_MS);
}
