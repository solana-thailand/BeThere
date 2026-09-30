//! Generative event posters (handoff F4): a risograph-style SVG seeded by the
//! event slug, so an event without an organizer poster still has its own face.
//!
//! Deterministic: the same slug always draws the same poster, on every device,
//! with no request and no storage. Colours come only from the brand palette
//! (`styles/style-01-core.css`); nothing here introduces a new one.
//!
//! Shapes are printed in two or three "inks" on paper, each layer multiplied
//! over the one below and nudged a few pixels off register, the way a
//! risograph misaligns its drums. The grain is the same fractal noise the
//! ticket card uses.

/// Poster canvas: 4:5, the ratio organizers upload posters in.
pub const POSTER_WIDTH: u32 = 400;
pub const POSTER_HEIGHT: u32 = 500;

/// Paper ground: `--paper`.
pub const PAPER: &str = "#ede7d6";

/// Inks: `--accent`, `--paper-ink`, `--info`, `--success`, `--warning`.
pub const INKS: [&str; 5] = ["#ff5c39", "#1e2447", "#3b82f6", "#2fbf71", "#f59e0b"];

/// FNV-1a over the slug: stable across platforms and builds, unlike
/// `std::hash`, which may change between Rust releases.
fn seed(slug: &str) -> u64 {
    slug.bytes().fold(0xcbf2_9ce4_8422_2325, |h, b| {
        (h ^ u64::from(b)).wrapping_mul(0x0000_0100_0000_01b3)
    })
}

/// xorshift64*: small, fast, and good enough to place a few shapes.
struct Rng(u64);

impl Rng {
    fn new(slug: &str) -> Self {
        // xorshift must not start at zero.
        Self(seed(slug).max(1))
    }

    fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_f491_4f6c_dd1d)
    }

    /// Uniform in `lo..=hi`.
    fn range(&mut self, lo: u32, hi: u32) -> u32 {
        lo + (self.next() % u64::from(hi - lo + 1)) as u32
    }
}

/// Two or three distinct inks, in print order.
fn pick_inks(rng: &mut Rng) -> Vec<&'static str> {
    let count = rng.range(2, 3) as usize;
    let mut pool = INKS.to_vec();
    (0..count)
        .map(|_| pool.remove(rng.next() as usize % pool.len()))
        .collect()
}

/// One shape in one ink, as an SVG element.
fn shape(rng: &mut Rng, ink: &str) -> String {
    let (w, h) = (POSTER_WIDTH, POSTER_HEIGHT);
    match rng.range(0, 3) {
        // A big disc, often bleeding off an edge.
        0 => {
            let r = rng.range(90, 190);
            let (cx, cy) = (rng.range(0, w), rng.range(0, h));
            format!(r#"<circle cx="{cx}" cy="{cy}" r="{r}" fill="{ink}"/>"#)
        }
        // A half-disc sitting on a horizontal line.
        1 => {
            let r = rng.range(80, 170);
            let (cx, cy) = (rng.range(r, w.saturating_sub(r).max(r)), rng.range(160, h));
            let (x0, x1) = (cx - r, cx + r);
            format!(r#"<path d="M{x0} {cy} A{r} {r} 0 0 1 {x1} {cy} Z" fill="{ink}"/>"#)
        }
        // A band of stripes, rotated.
        2 => {
            let gap = rng.range(14, 26);
            let angle = rng.range(0, 3) * 45;
            let y = rng.range(40, h - 160);
            let stripes: String = (0..6)
                .map(|i| {
                    let sy = y + i * gap;
                    format!(
                        r#"<rect x="-100" y="{sy}" width="600" height="{}"/>"#,
                        gap / 2
                    )
                })
                .collect();
            format!(
                r#"<g fill="{ink}" transform="rotate({angle} {} {})">{stripes}</g>"#,
                w / 2,
                h / 2
            )
        }
        // A tilted block.
        _ => {
            let (bw, bh) = (rng.range(120, 260), rng.range(60, 180));
            let (x, y) = (rng.range(0, w - 60), rng.range(0, h - 60));
            let angle = rng.range(0, 30) as i32 - 15;
            format!(
                r#"<rect x="{x}" y="{y}" width="{bw}" height="{bh}" fill="{ink}" transform="rotate({angle} {x} {y})"/>"#
            )
        }
    }
}

/// The poster for `slug`, as a standalone SVG document.
pub fn poster_svg(slug: &str) -> String {
    let mut rng = Rng::new(slug);
    let inks = pick_inks(&mut rng);
    let layers: String = inks
        .iter()
        .map(|ink| {
            let shapes: String = (0..rng.range(1, 2)).map(|_| shape(&mut rng, ink)).collect();
            // Off register by a few pixels, like a riso drum.
            let (dx, dy) = (rng.range(0, 6) as i32 - 3, rng.range(0, 6) as i32 - 3);
            format!(
                r#"<g style="mix-blend-mode:multiply" opacity="0.9" transform="translate({dx} {dy})">{shapes}</g>"#
            )
        })
        .collect();
    format!(
        concat!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {w} {h}" width="{w}" height="{h}">"#,
            r#"<filter id="g"><feTurbulence type="fractalNoise" baseFrequency="0.9" numOctaves="2" stitchTiles="stitch"/></filter>"#,
            r#"<rect width="{w}" height="{h}" fill="{paper}"/>{layers}"#,
            r#"<rect width="{w}" height="{h}" filter="url(#g)" opacity="0.08"/></svg>"#
        ),
        w = POSTER_WIDTH,
        h = POSTER_HEIGHT,
        paper = PAPER,
        layers = layers,
    )
}

/// The poster as a `data:` URL for an `<img src>`. Only the characters that
/// break a URL are escaped (`#`, `%`, quotes, angle brackets), which keeps it
/// far smaller than base64. The CSP allows `img-src data:`.
pub fn poster_data_url(slug: &str) -> String {
    let svg = poster_svg(slug);
    let mut out = String::with_capacity(svg.len() + 64);
    out.push_str("data:image/svg+xml;charset=utf-8,");
    for c in svg.chars() {
        match c {
            '#' => out.push_str("%23"),
            '%' => out.push_str("%25"),
            '<' => out.push_str("%3C"),
            '>' => out.push_str("%3E"),
            '"' => out.push('\''),
            _ => out.push(c),
        }
    }
    out
}
