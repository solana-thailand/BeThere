//! Draw and upload the event's 1200×630 share card on save (`.issues/183`
//! option B). The Worker names it in the `og:image` of `/e/{slug}` when the
//! event has no raster poster.
//!
//! Fire and forget: the save has already succeeded when this runs, and a
//! failure here (fonts, a tainted canvas, the upload) is logged and never
//! shown as a save error. The layout rules are in `utils/og_card.rs`.
//!
//! The canvas bindings (`web-sys` `HtmlCanvasElement`, …) are enabled by the
//! `staff` feature only, so the attendee shell never links them; without it
//! [`refresh`] is a no-op.

use crate::utils::og_card::CardEvent;

/// Redraw and upload the card for `event_id` in the background.
#[cfg(feature = "staff")]
pub(super) fn refresh(event_id: String, event: CardEvent) {
    leptos::task::spawn_local(async move {
        match draw::render_png(&event).await {
            Ok(png) => match crate::api::upload_og_card(&event_id, &png).await {
                Ok(_) => log::info!("[event-form] share card uploaded for {event_id}"),
                Err(e) => log::warn!("[event-form] share card upload failed: {e}"),
            },
            Err(e) => log::warn!("[event-form] share card not drawn: {e:?}"),
        }
    });
}

/// The attendee shell has no event editor; nothing to draw.
#[cfg(not(feature = "staff"))]
pub(super) fn refresh(_event_id: String, _event: CardEvent) {}

#[cfg(feature = "staff")]
mod draw {
    use wasm_bindgen::JsCast;
    use wasm_bindgen::prelude::*;
    use wasm_bindgen_futures::JsFuture;
    use web_sys::{CanvasRenderingContext2d, HtmlCanvasElement, HtmlImageElement};

    use crate::utils::og_card::{
        ART, ArtSource, BAR, CardEvent, FIELD, INK, INK_2, LINE_FONT, LOGO_FONT, LOGO_Y, OG_HEIGHT,
        OG_WIDTH, PAPER, PLACE_Y, TEXT_MAX_W, TEXT_X, TITLE_FONT, TITLE_LINE_H, TITLE_Y, WHEN_Y,
        art_source, card_text, cover_crop,
    };

    /// The finished card as a PNG blob.
    pub(super) async fn render_png(event: &CardEvent) -> Result<web_sys::Blob, JsValue> {
        let document = web_sys::window()
            .and_then(|w| w.document())
            .ok_or("no document")?;
        let canvas: HtmlCanvasElement = document.create_element("canvas")?.dyn_into()?;
        canvas.set_width(OG_WIDTH);
        canvas.set_height(OG_HEIGHT);
        let ctx: CanvasRenderingContext2d = canvas
            .get_context("2d")?
            .ok_or("no 2d context")?
            .dyn_into()?;

        // Load the faces for the glyphs this card uses: Google Fonts serves
        // Anuphan per script, so the Thai subset loads only if asked for.
        let all_text = format!("BeThere {} {}", event.name, event.location);
        let loads = js_sys::Array::new();
        for font in [LOGO_FONT, TITLE_FONT, LINE_FONT] {
            loads.push(&document.fonts().load_with_text(font, &all_text));
        }
        // A face that will not load still draws in a fallback font, and one
        // that never settles must not stall the card: `fonts.load` stayed
        // pending for 30 s+ in Chrome after a navigation cut a font fetch
        // short, and the card was then never uploaded (`.issues/183`).
        let _ = JsFuture::from(js_sys::Promise::race(&js_sys::Array::of2(
            &js_sys::Promise::all_settled(&loads),
            &delay(FONT_WAIT_MS),
        )))
        .await;

        ctx.set_fill_style_str(PAPER);
        ctx.fill_rect(0.0, 0.0, f64::from(OG_WIDTH), f64::from(OG_HEIGHT));
        draw_art(&ctx, event).await;
        draw_text(&ctx, event)?;
        ctx.set_fill_style_str(FIELD);
        ctx.fill_rect(BAR.x, BAR.y, BAR.w, BAR.h);

        to_png(&canvas).await
    }

    fn draw_text(ctx: &CanvasRenderingContext2d, event: &CardEvent) -> Result<(), JsValue> {
        ctx.set_text_baseline("alphabetic");
        let fits_in = |font: &'static str| {
            move |text: &str| {
                ctx.set_font(font);
                ctx.measure_text(text)
                    .map(|m| m.width() <= TEXT_MAX_W)
                    .unwrap_or(false)
            }
        };
        let fits_title = fits_in(TITLE_FONT);
        let fits_line = fits_in(LINE_FONT);
        let text = card_text(event, &fits_title, &fits_line);

        ctx.set_fill_style_str(INK);
        ctx.set_font(LOGO_FONT);
        ctx.fill_text("BeThere", TEXT_X, LOGO_Y)?;
        ctx.set_font(TITLE_FONT);
        for (i, line) in text.title.iter().enumerate() {
            ctx.fill_text(line, TEXT_X, TITLE_Y + TITLE_LINE_H * i as f64)?;
        }
        ctx.set_fill_style_str(INK_2);
        ctx.set_font(LINE_FONT);
        ctx.fill_text(&text.when, TEXT_X, WHEN_Y)?;
        if let Some(place) = &text.place {
            ctx.fill_text(place, TEXT_X, PLACE_Y)?;
        }
        Ok(())
    }

    /// The organizer's poster, else (or if it fails to load) the generative one.
    async fn draw_art(ctx: &CanvasRenderingContext2d, event: &CardEvent) {
        let generative = crate::utils::poster::poster_data_url(&event.slug);
        let image = match art_source(&event.poster_url) {
            ArtSource::Uploaded(url) => match load_image(&url).await {
                Ok(img) => Some(img),
                Err(_) => load_image(&generative).await.ok(),
            },
            ArtSource::Generative => load_image(&generative).await.ok(),
        };
        let Some(img) = image else { return };
        let (sx, sy, sw, sh) = cover_crop(
            f64::from(img.natural_width()),
            f64::from(img.natural_height()),
            ART.w,
            ART.h,
        );
        ctx.save();
        ctx.set_shadow_color("rgba(18, 18, 18, 0.25)");
        ctx.set_shadow_blur(48.0);
        ctx.set_shadow_offset_y(20.0);
        ctx.set_fill_style_str(PAPER);
        ctx.fill_rect(ART.x, ART.y, ART.w, ART.h);
        ctx.restore();
        let _ = ctx.draw_image_with_html_image_element_and_sw_and_sh_and_dx_and_dy_and_dw_and_dh(
            &img, sx, sy, sw, sh, ART.x, ART.y, ART.w, ART.h,
        );
    }

    /// Longest wait for the card's fonts before drawing with what is loaded.
    const FONT_WAIT_MS: i32 = 3_000;

    /// A promise that resolves after `ms` milliseconds.
    fn delay(ms: i32) -> js_sys::Promise {
        js_sys::Promise::new(&mut |resolve, _reject| {
            if let Some(window) = web_sys::window() {
                let _ = window.set_timeout_with_callback_and_timeout_and_arguments_0(&resolve, ms);
            }
        })
    }

    async fn load_image(src: &str) -> Result<HtmlImageElement, JsValue> {
        let img = HtmlImageElement::new()?;
        img.set_src(src);
        JsFuture::from(img.decode()).await?;
        Ok(img)
    }

    /// `canvas.toBlob(cb, "image/png")` as a future. A tainted canvas throws
    /// here, which the caller logs.
    async fn to_png(canvas: &HtmlCanvasElement) -> Result<web_sys::Blob, JsValue> {
        let mut failed: Option<JsValue> = None;
        let promise = js_sys::Promise::new(&mut |resolve, reject| {
            let callback = Closure::once_into_js(move |blob: JsValue| {
                let _ = match blob.is_null() {
                    true => reject.call1(&JsValue::NULL, &JsValue::from_str("toBlob gave null")),
                    false => resolve.call1(&JsValue::NULL, &blob),
                };
            });
            if let Err(e) = canvas.to_blob_with_type(callback.unchecked_ref(), "image/png") {
                failed = Some(e);
            }
        });
        if let Some(e) = failed {
            return Err(e);
        }
        JsFuture::from(promise).await?.dyn_into()
    }
}
