//! Video/livestream embed section for the ticket page.

use leptos::prelude::*;

/// Extract YouTube embed URL from various YouTube link formats.
///
/// Supports: watch?v=, youtu.be/, /live/, /shorts/, /embed/
/// Returns empty string if not a YouTube URL or video ID can't be extracted.
pub fn youtube_embed_url(url: &str) -> String {
    if !url.contains("youtube.com") && !url.contains("youtu.be") {
        return String::new();
    }
    let vid = if url.contains("youtu.be/") {
        url.split("youtu.be/")
            .nth(1)
            .map(|s| s.split('?').next().unwrap_or(""))
            .unwrap_or("")
    } else if url.contains("v=") {
        url.split("v=")
            .nth(1)
            .map(|s| s.split('&').next().unwrap_or(""))
            .unwrap_or("")
    } else if url.contains("/live/") {
        url.split("/live/")
            .nth(1)
            .map(|s| s.split('?').next().unwrap_or(""))
            .unwrap_or("")
    } else if url.contains("/shorts/") {
        url.split("/shorts/")
            .nth(1)
            .map(|s| s.split('?').next().unwrap_or(""))
            .unwrap_or("")
    } else if url.contains("/embed/") {
        url.split("/embed/")
            .nth(1)
            .map(|s| s.split('?').next().unwrap_or(""))
            .unwrap_or("")
    } else {
        ""
    };
    if vid.is_empty() {
        String::new()
    } else {
        format!("https://www.youtube.com/embed/{vid}")
    }
}

/// Video/livestream embed section — renders YouTube iframe or plain link fallback.
#[component]
pub fn VideoSection(
    /// Original video URL (for "Watch on YouTube" link)
    #[prop(into)]
    video_url: String,
    /// CSS class modifier (empty string = default, "card" = card wrapper)
    #[prop(optional, into)]
    variant: Option<String>,
) -> impl IntoView {
    let embed_url = youtube_embed_url(&video_url);
    let has_embed = !embed_url.is_empty();

    let wrapper_class = if variant.as_deref() == Some("card") {
        "card ticket-video-card"
    } else {
        "ticket-video-section"
    };

    view! {
        <div class=wrapper_class>
            <h3 class="ticket-video-heading">
                "📺 Livestream / Recording"
            </h3>
            {if has_embed {
                let link = video_url.clone();
                view! {
                    <div class="ticket-video-embed-wrapper">
                        <iframe
                            src=embed_url
                            class="ticket-video-iframe"
                            allow="accelerometer; autoplay; clipboard-write; encrypted-media; gyroscope; picture-in-picture"
                            allowfullscreen=true
                            title="Event video"
                        />
                    </div>
                    <a
                        href=link
                        target="_blank"
                        rel="noopener noreferrer"
                        class="btn btn-outline btn-sm ticket-video-link"
                    >
                        "Watch on YouTube →"
                    </a>
                }.into_any()
            } else {
                view! {
                    <a
                        href=video_url
                        target="_blank"
                        rel="noopener noreferrer"
                        class="btn btn-outline btn-sm ticket-video-link"
                    >
                        "Watch Video →"
                    </a>
                }.into_any()
            }}
        </div>
    }
}
