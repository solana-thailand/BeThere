/**
 * The lit room (.plans/045 R4.1, owner 6-7 Oct): the RTM #6 room as a
 * 320x180 grey image, dithered live (Bayer 4x4) on the hero's canvas. It
 * sits dim; the pointer or a finger is a light, and under it more of the room
 * comes on. Nothing moves on its own; under reduced motion the room is drawn
 * once, dim. Ported from the prototype (bethere-ux/site/site.js `lit_room`;
 * the image is scripts/site_room.py's, in the devrel-helper repo).
 *
 * Not part of the first load: copy-dir puts it in dist/room/ with no tag,
 * and src/pages/landing/room.rs injects it (with ?v=) once the hero is in
 * the page. It then calls
 *
 *   window.bethereRoom.mount(area, canvas)
 *
 * `area` takes the pointer; each child of its `.lp-hero-text` is a quiet box
 * where the light keeps 15% of its strength, so the type never washes out.
 * The canvas is removed if the image cannot load.
 *
 * Not ported yet: the prototype's lamps (one light per course episode
 * watched) wait for courses (R4.4).
 */
(function () {
  "use strict";

  var W = 320;
  var H = 180;
  var IMG_URL = "/room/rtm6-room.webp?v=1";
  var BAYER = [0, 8, 2, 10, 12, 4, 14, 6, 3, 11, 1, 9, 15, 7, 13, 5].map(function (b) {
    return (b + 0.5) / 16;
  });
  // The dim dots run the Solana gradient left to right (purple -> green,
  // darkened so the type on it holds); where the light falls the people come
  // on in warm, the colour of people on this site.
  var SOL_A = [86, 40, 150];
  var SOL_B = [14, 120, 96];
  var WARM = [255, 122, 92];
  var NIGHT = [14, 16, 32];
  var BASE = 0.62;
  var R = 46;
  var QUIET_EDGE = 4;
  var DIMS = [];
  for (var x = 0; x < W; x++) {
    DIMS.push(
      SOL_A.map(function (a, k) {
        return Math.round(a + ((SOL_B[k] - a) * x) / (W - 1));
      })
    );
  }

  function mount(area, cv) {
    if (!area || !cv || !cv.getContext) return;
    var reduced = matchMedia("(prefers-reduced-motion: reduce)").matches;
    var cx = cv.getContext("2d");
    var out = cx.createImageData(W, H);
    var quiet = new Float32Array(W * H);
    var lum = null;
    var light = { x: -99, y: H * 0.5, k: 0 };
    var want = { x: -99, y: H * 0.5, k: 0 };
    var raf = 0;

    // pointer -> canvas cell, through object-fit: cover and object-position: center 40%
    function to_cell(px, py) {
      var b = cv.getBoundingClientRect();
      var k = Math.max(b.width / W, b.height / H);
      return {
        x: (px - b.left - (b.width - W * k) / 2) / k,
        y: (py - b.top - (b.height - H * k) * 0.4) / k,
      };
    }

    function place_quiet() {
      quiet.fill(0);
      var lines = area.querySelectorAll(".lp-hero-text > *");
      for (var n = 0; n < lines.length; n++) {
        var r = lines[n].getBoundingClientRect();
        if (!r.width) continue;
        var a = to_cell(r.left - 12, r.top - 8);
        var b = to_cell(r.right + 12, r.bottom + 8);
        var y1 = Math.min(H, Math.ceil(b.y + QUIET_EDGE));
        var x1 = Math.min(W, Math.ceil(b.x + QUIET_EDGE));
        for (var y = Math.max(0, Math.floor(a.y - QUIET_EDGE)); y < y1; y++) {
          for (var x = Math.max(0, Math.floor(a.x - QUIET_EDGE)); x < x1; x++) {
            var edge = Math.max(a.x - x, x - b.x, a.y - y, y - b.y, 0);
            var i = y * W + x;
            quiet[i] = Math.max(quiet[i], 1 - edge / QUIET_EDGE);
          }
        }
      }
    }

    function draw() {
      if (!lum) return;
      var d = out.data;
      var r2 = 2 * R * R;
      for (var y = 0, i = 0; y < H; y++) {
        for (var x = 0; x < W; x++, i++) {
          var dx = x - light.x;
          var dy = y - light.y;
          var g = light.k * Math.exp(-(dx * dx + dy * dy) / r2) * (1 - 0.85 * quiet[i]);
          var c = NIGHT;
          if (lum[i] * (BASE + g) > BAYER[(y & 3) * 4 + (x & 3)]) {
            // the warm comes in with the light, blended over the dim colour
            var t = Math.min(1, Math.max(0, (g - 0.2) * 1.6)) * 0.8;
            var dim = DIMS[x];
            c = t
              ? [
                  Math.round(dim[0] + (WARM[0] - dim[0]) * t),
                  Math.round(dim[1] + (WARM[1] - dim[1]) * t),
                  Math.round(dim[2] + (WARM[2] - dim[2]) * t),
                ]
              : dim;
          }
          d[i * 4] = c[0];
          d[i * 4 + 1] = c[1];
          d[i * 4 + 2] = c[2];
          d[i * 4 + 3] = 255;
        }
      }
      cx.putImageData(out, 0, 0);
    }

    function tick() {
      light.x += (want.x - light.x) * 0.25;
      light.y += (want.y - light.y) * 0.25;
      light.k += (want.k - light.k) * 0.12;
      draw();
      var moving =
        Math.abs(want.x - light.x) + Math.abs(want.y - light.y) + Math.abs(want.k - light.k) > 0.01;
      raf = moving ? requestAnimationFrame(tick) : 0;
    }

    function go() {
      if (!raf) raf = requestAnimationFrame(tick);
    }

    function aim(e) {
      var c = to_cell(e.clientX, e.clientY);
      want.x = c.x;
      want.y = c.y;
      want.k = 0.65;
      go();
    }

    function on_resize() {
      // the hero is gone (the router moved on): stop listening
      if (!cv.isConnected) {
        removeEventListener("resize", on_resize);
        return;
      }
      place_quiet();
      draw();
    }

    if (!reduced) {
      area.addEventListener("pointermove", aim);
      area.addEventListener("pointerdown", aim);
      area.addEventListener("pointerleave", function () {
        want.k = 0;
        go();
      });
    }

    var img = new Image();
    img.onload = function () {
      var t = document.createElement("canvas");
      t.width = W;
      t.height = H;
      var tc = t.getContext("2d");
      tc.drawImage(img, 0, 0, W, H);
      var px = tc.getImageData(0, 0, W, H).data;
      lum = new Float32Array(W * H);
      for (var i = 0; i < W * H; i++) lum[i] = px[i * 4] / 255;
      place_quiet();
      draw();
      cv.classList.add("lp-room-on");
      addEventListener("resize", on_resize);
    };
    img.onerror = function () {
      cv.remove();
    };
    img.src = IMG_URL;
  }

  window.bethereRoom = { mount: mount };
})();
