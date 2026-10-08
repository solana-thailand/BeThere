/**
 * The landing goal globe (.plans/043 L10): Solana events of the last 12
 * months as dots on a turning globe, drag to turn, search or tap a country to
 * list its events. Ported from the prototype (bethere-ux/landing.html).
 *
 * Not part of the first load: copy-dir puts it in dist/globe/ with no tag,
 * and src/pages/landing/goal.rs injects it (with ?v=) only when the goal
 * section scrolls near, after fetching globe-data.json itself. It then calls
 *
 *   window.bethereGlobe.mount(root, dataText)
 *
 * `root` holds a <canvas>, an <input list=…>, a <datalist> and a panel
 * <div>, each marked with data-globe="…". The root's data-lang ("th" / "en")
 * and data-ours (BeThere's events, from /api/public/stats) are read on every
 * frame, so the language switch and a late stats response just show up.
 *
 * Names and dots only (build plan rule 5): no logos, no links out.
 */
(function () {
  "use strict";

  var S = 440;
  var RAD = Math.PI / 180;
  var INK = "#121212";
  var PAPER = "#f4f0e6";
  var BRAND = "#6b63f0";
  var PANEL_ROWS = 6;

  var TEXT = {
    en: {
      home: function (n) {
        return n ? "Bangkok · " + n + " events on BeThere" : "Bangkok · BeThere";
      },
      meta: function (n, ahead) {
        return n + " Solana events · " + ahead + " upcoming";
      },
      times: function (n) {
        return n + " times";
      },
      more: function (n) {
        return "+ " + n + " more";
      },
    },
    th: {
      home: function (n) {
        return n ? "กรุงเทพ · " + n + " งานบน BeThere" : "กรุงเทพ · BeThere";
      },
      meta: function (n, ahead) {
        return n + " งาน Solana · กำลังจะมา " + ahead;
      },
      times: function (n) {
        return n + " ครั้ง";
      },
      more: function (n) {
        return "อีก " + n + " งาน";
      },
    },
  };

  function text(root) {
    return root.dataset.lang === "th" ? TEXT.th : TEXT.en;
  }

  function el(tag, cls, content) {
    var e = document.createElement(tag);
    if (cls) e.className = cls;
    if (content != null) e.textContent = content;
    return e;
  }

  function mount(root, dataText) {
    if (root.__globeMounted) return;
    root.__globeMounted = true;
    var G = JSON.parse(dataText);
    var cv = root.querySelector('[data-globe="canvas"]');
    var sel = root.querySelector('[data-globe="search"]');
    var list = root.querySelector('[data-globe="countries"]');
    var panel = root.querySelector('[data-globe="panel"]');
    var cx = cv.getContext("2d");
    var dpr = Math.min(2, window.devicePixelRatio || 1);
    cv.width = S * dpr;
    cv.height = S * dpr;
    cx.scale(dpr, dpr);
    var R = S / 2 - 8;
    var max = Math.max.apply(
      null,
      G.events.map(function (e) {
        return e[2];
      }),
    );
    var C = G.countries || {};
    var codes = Object.keys(C)
      .filter(function (k) {
        return C[k].lat != null;
      })
      .sort(function (a, b) {
        return C[b].events.length - C[a].events.length;
      });
    // View: the longitude facing us, the tilt, a picked country.
    var lon0 = G.home[1] - 25,
      tilt = 12,
      picked = null,
      held = false,
      glide = null,
      spin = 0,
      lastX = 0,
      lastT = 0,
      running = false,
      start = null;

    function proj(lat, lon) {
      var p = lat * RAD,
        l = (lon - lon0) * RAD,
        t0 = tilt * RAD;
      var z = Math.sin(t0) * Math.sin(p) + Math.cos(t0) * Math.cos(p) * Math.cos(l);
      return [
        S / 2 + R * Math.cos(p) * Math.sin(l),
        S / 2 - R * (Math.cos(t0) * Math.sin(p) - Math.sin(t0) * Math.cos(p) * Math.cos(l)),
        z,
      ];
    }

    function drawHome(phase) {
      var h = proj(G.home[0], G.home[1]);
      if (h[2] <= 0) return;
      var hx = h[0],
        hy = h[1];
      var lx = S - 12,
        ly = 34;
      var label = text(root).home(root.dataset.ours).toUpperCase();
      cx.save();
      cx.globalAlpha = Math.min(1, h[2] * 3);
      // A leader from our city to the label, dashes flowing towards the label.
      cx.setLineDash([6, 6]);
      cx.lineDashOffset = -phase * 24;
      cx.strokeStyle = BRAND;
      cx.lineWidth = 2;
      cx.beginPath();
      cx.moveTo(hx, hy);
      cx.quadraticCurveTo(hx + 30, ly + 10, lx - 8, ly + 18);
      cx.stroke();
      cx.setLineDash([]);
      // canvas takes a real font list, not a CSS variable
      cx.font = "600 13px Inter, Anuphan, sans-serif";
      var w = Math.min(lx - 4, cx.measureText(label).width + 24 + 26);
      cx.fillStyle = BRAND;
      cx.beginPath();
      if (cx.roundRect) cx.roundRect(lx - w, ly, w, 28, 3);
      else cx.rect(lx - w, ly, w, 28);
      cx.fill();
      cx.fillStyle = "#ffffff";
      cx.textAlign = "right";
      cx.fillText(label, lx - 12, ly + 19);
      // Thailand: five stripes, the blue one double.
      var fx = lx - w + 10,
        fy = ly + 8,
        u = 2;
      [
        ["#a51931", 1],
        ["#f4f5f8", 1],
        ["#2d2a4a", 2],
        ["#f4f5f8", 1],
        ["#a51931", 1],
      ].reduce(function (y, s) {
        cx.fillStyle = s[0];
        cx.fillRect(fx, y, 18, s[1] * u);
        return y + s[1] * u;
      }, fy);
      var r = 7 + 10 * (phase % 1);
      cx.beginPath();
      cx.arc(hx, hy, r, 0, 7);
      cx.strokeStyle = "rgba(107, 99, 240, " + (1 - (phase % 1)) + ")";
      cx.stroke();
      cx.beginPath();
      cx.arc(hx, hy, 7, 0, 7);
      cx.fillStyle = BRAND;
      cx.fill();
      cx.lineWidth = 3;
      cx.strokeStyle = "#12152a";
      cx.stroke();
      cx.restore();
    }

    function draw(phase) {
      phase = phase || 0;
      cx.clearRect(0, 0, S, S);
      cx.beginPath();
      cx.arc(S / 2, S / 2, R, 0, 7);
      cx.fillStyle = INK;
      cx.fill();
      cx.strokeStyle = "rgba(244,240,230,.22)";
      cx.stroke();
      cx.fillStyle = "rgba(244,240,230,.2)";
      for (var i = 0; i < G.land.length; i++) {
        var a = proj(G.land[i][0], G.land[i][1]);
        if (a[2] > 0) cx.fillRect(a[0] - 1.2, a[1] - 1.2, 2.4, 2.4);
      }
      cx.fillStyle = "rgba(244, 240, 230, .85)";
      for (var j = 0; j < G.events.length; j++) {
        var e = G.events[j],
          b = proj(e[0], e[1]);
        if (b[2] <= 0) continue;
        cx.beginPath();
        cx.arc(b[0], b[1], 2.5 + 7 * Math.sqrt(e[2] / max), 0, 7);
        cx.fill();
      }
      drawHome(phase);
      if (picked && C[picked]) {
        var p = proj(C[picked].lat, C[picked].lng);
        if (p[2] > 0) {
          cx.save();
          cx.lineWidth = 3;
          cx.strokeStyle = BRAND;
          cx.beginPath();
          cx.arc(p[0], p[1], 14, 0, 7);
          cx.stroke();
          cx.font = "700 13px Inter, Anuphan, sans-serif";
          cx.fillStyle = PAPER;
          cx.textAlign = "center";
          cx.fillText(C[picked].name, p[0], p[1] + 30);
          cx.restore();
        }
      }
    }

    // Country panel: newest first, repeats of one event folded into a date range.
    function showPanel(cc) {
      var t = text(root),
        c = C[cc];
      var ahead = c.events.filter(function (e) {
        return e[3];
      }).length;
      var rows = [];
      c.events.forEach(function (e) {
        var last = rows[rows.length - 1];
        if (last && last.n === e[0]) {
          last.from = e[1];
          last.count++;
        } else rows.push({ n: e[0], to: e[1], from: e[1], city: e[2], count: 1 });
      });
      panel.replaceChildren();
      panel.appendChild(el("h3", null, c.name));
      panel.appendChild(el("p", "lp-globe-meta", t.meta(c.events.length, ahead)));
      var ul = el("ul");
      rows.slice(0, PANEL_ROWS).forEach(function (r) {
        var li = el("li");
        li.appendChild(el("span", null, r.n));
        var when = r.count > 1 ? r.from + " – " + r.to + " · " + t.times(r.count) : r.to;
        li.appendChild(el("small", null, when + (r.city ? " · " + r.city : "")));
        ul.appendChild(li);
      });
      panel.appendChild(ul);
      if (rows.length > PANEL_ROWS) {
        panel.appendChild(el("p", "lp-globe-meta", t.more(rows.length - PANEL_ROWS)));
      }
      panel.hidden = false;
    }

    function pick(cc) {
      picked = cc || null;
      sel.value = cc ? C[cc].name : "";
      if (!picked) {
        panel.hidden = true;
        return;
      }
      showPanel(cc);
      glide = { lon: C[cc].lng, tilt: Math.max(-40, Math.min(50, C[cc].lat * 0.6)) };
      if (!running) draw();
    }

    codes.forEach(function (k) {
      var o = el("option");
      o.value = C[k].name;
      o.label = String(C[k].events.length);
      list.appendChild(o);
    });
    var byName = {};
    codes.forEach(function (k) {
      byName[C[k].name.toLowerCase()] = k;
    });
    function fromBox() {
      var v = sel.value.trim().toLowerCase();
      if (!v) return;
      var cc =
        byName[v] ||
        codes.find(function (k) {
          return k.toLowerCase() === v || C[k].name.toLowerCase().indexOf(v) === 0;
        });
      if (cc) pick(cc);
    }
    sel.addEventListener("change", fromBox);
    sel.addEventListener("keydown", function (e) {
      if (e.key === "Enter") fromBox();
    });

    // Drag to turn; a tap without movement picks the nearest country dot.
    cv.addEventListener("pointerdown", function (e) {
      cv.setPointerCapture(e.pointerId);
      start = { x: e.clientX, y: e.clientY, lon: lon0, tilt: tilt, moved: false };
      held = true;
      glide = null;
      spin = 0;
      lastX = e.clientX;
      lastT = performance.now();
      cv.classList.add("lp-dragging");
    });
    cv.addEventListener("pointermove", function (e) {
      if (!start) return;
      var k = S / cv.getBoundingClientRect().width;
      var dx = (e.clientX - start.x) * k,
        dy = (e.clientY - start.y) * k;
      if (Math.abs(dx) + Math.abs(dy) > 4) start.moved = true;
      lon0 = start.lon - dx * 0.45;
      tilt = Math.max(-50, Math.min(60, start.tilt + dy * 0.3));
      var now = performance.now();
      if (now > lastT) {
        spin = (-(e.clientX - lastX) * k * 0.45) / ((now - lastT) / 1000);
        lastX = e.clientX;
        lastT = now;
      }
      // Dragging away from a focused country lets go of it; the panel stays.
      if (start.moved && picked) picked = null;
      if (!running) draw();
    });
    function end(e) {
      if (!start) return;
      held = false;
      cv.classList.remove("lp-dragging");
      if (!start.moved) {
        var b = cv.getBoundingClientRect(),
          k = S / b.width;
        var x = (e.clientX - b.left) * k,
          y = (e.clientY - b.top) * k;
        var best = null,
          bd = 28;
        codes.forEach(function (cc) {
          var p = proj(C[cc].lat, C[cc].lng);
          var d = Math.hypot(p[0] - x, p[1] - y);
          if (p[2] > 0 && d < bd) {
            bd = d;
            best = cc;
          }
        });
        pick(best);
      }
      // Held still before letting go: no fling.
      if (start.moved && performance.now() - lastT > 80) spin = 0;
      start = null;
      if (!running) draw();
    }
    cv.addEventListener("pointerup", end);
    cv.addEventListener("pointercancel", end);

    // A full turn every 40 s on its own (9° a second); a fling carries on and
    // eases back to that; a picked country is glided to and held.
    if (window.matchMedia("(prefers-reduced-motion: reduce)").matches) {
      draw();
      return;
    }
    var last = performance.now();
    var seen = new IntersectionObserver(function (es) {
      es.forEach(function (e) {
        running = e.isIntersecting;
        if (running) {
          last = performance.now();
          requestAnimationFrame(tick);
        }
      });
    });
    function tick(now) {
      // The landing went away (SPA navigation): stop for good.
      if (!cv.isConnected) {
        running = false;
        seen.disconnect();
        return;
      }
      if (!running) return;
      var dt = (now - last) / 1000;
      last = now;
      if (glide) {
        var dl = ((glide.lon - lon0 + 540) % 360) - 180;
        lon0 += dl * Math.min(1, dt * 4);
        tilt += (glide.tilt - tilt) * Math.min(1, dt * 4);
        if (Math.abs(dl) < 0.2) glide = null;
      } else if (!held && !picked) {
        spin += (9 - spin) * Math.min(1, dt * 1.5);
        lon0 = (lon0 + dt * spin) % 360;
      }
      draw(now / 1200);
      requestAnimationFrame(tick);
    }
    seen.observe(cv);
    draw();
  }

  window.bethereGlobe = { mount: mount };
})();
