//! Every site page opens in the still room (.plans/045, the prototype's
//! `.dark-head`): one `PageHead` per page, which holds the page's only `h1`,
//! so the sections it reuses from the landing stay `h2`.

fn read(rel: &str) -> String {
    std::fs::read_to_string(format!("{}/{rel}", env!("CARGO_MANIFEST_DIR"))).unwrap()
}

#[test]
fn each_site_page_has_one_head() {
    for page in ["events", "organizers", "sponsors"] {
        let src = read(&format!("src/pages/site/{page}.rs"));
        assert_eq!(src.matches("<PageHead").count(), 1, "{page}.rs");
        assert!(!src.contains("<h1"), "{page}.rs: the h1 is the head's");
    }
}

#[test]
fn reused_sections_never_add_an_h1() {
    for section in ["how", "sponsors"] {
        let src = read(&format!("src/pages/landing/{section}.rs"));
        assert!(!src.contains("<h1"), "landing/{section}.rs");
    }
    assert_eq!(read("src/pages/site/head.rs").matches("<h1").count(), 1);
}

#[test]
fn the_still_room_is_shipped_and_lazy() {
    let css = read("styles/style-23-landing.css");
    let url = css
        .split("url(\"/room/")
        .nth(1)
        .and_then(|rest| rest.split(['?', '"']).next())
        .expect("the head's background url");
    let image = std::fs::read(format!("{}/room/{url}", env!("CARGO_MANIFEST_DIR"))).unwrap();
    assert_eq!(&image[8..12], b"WEBP");
    // Only a page head pulls it: never a tag in index.html.
    assert!(!read("index.html").contains(url));
}
