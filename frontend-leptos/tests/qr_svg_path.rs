//! Inline SVG QR for the landing's ticket expand (.plans/038 P2-a).

use event_checkin_frontend::utils::qr_gen::qr_svg_path;

#[test]
fn side_includes_the_quiet_zone_and_every_module_is_drawn() {
    // "hello" encodes as a version-1 QR: 21 modules + 4 quiet each side.
    let (side, d) = qr_svg_path("hello").expect("encodes");
    assert_eq!(side, 29);
    // Each run is one closed subpath; nothing is drawn in the quiet zone.
    assert!(d.starts_with('M') && d.ends_with('z'));
    for run in d.split('z').filter(|r| !r.is_empty()) {
        let coords: Vec<u32> = run[1..]
            .split(['h', ' '])
            .take(2)
            .map(|n| n.parse().unwrap())
            .collect();
        assert!(
            (4..25).contains(&coords[0]) && (4..25).contains(&coords[1]),
            "{run}"
        );
    }
}

#[test]
fn a_check_in_url_encodes_and_oversized_input_does_not() {
    let url = "https://bethere.example/checkin/0190e2e0-0000-7000-8000-000000000001";
    assert!(qr_svg_path(url).is_some());
    assert!(qr_svg_path(&"x".repeat(8000)).is_none());
}
