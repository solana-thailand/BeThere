//! `/feedback` — one page that collects feedback for every event the signed-in
//! person attended (`.issues/091`).
//!
//! The unit of the post-event form is the event, so a person who came to three
//! Road to Mainnet sessions gets three inbox rows and three forms. 30 people
//! own the 45 outstanding survey notifications in production; asking them to
//! fill three near-identical forms is how a 45-response campaign becomes a
//! 15-response one.
//!
//! This page keeps the per-event answers — which is the signal DevRel reports
//! on — and collapses only the navigation: one question block per event, one
//! submit, N calls to `POST /api/public/event/{slug}/register-post-event`.
//!
//! **Eligibility is not re-derived here.** The list comes from
//! `GET /api/my-notifications`, whose `notification_inbox_visible` view already
//! encodes exactly the right rule: enrolled, approved, has a `checked_in_at`,
//! and the event's form still accepts. Recomputing that in the client would be
//! a second definition to keep in step.

use leptos::prelude::*;
use serde::Deserialize;

use crate::api::{PostEventRegisterBody, register_post_event};

/// The question set, transcribed from the two Google Forms DevRel has been
/// running (`solana-thailand-devrel-helper`, `reports/phase-1/survey-emails.md`).
///
/// Keeping their wording matters more than improving it: the onsite form's
/// four satisfaction dimensions are what the Phase 1 report is written against,
/// so a prettier scale here would produce numbers that cannot be compared with
/// the ones already submitted to the Foundation.
///
/// The `post.` prefix routes every answer to `registration_responses` scoped to
/// the event and skips the developer-profile upsert (`.issues/082`).
const DIMENSIONS: [(&str, &str); 4] = [
    ("post.satisfaction.content", "ด้านเนื้อหา"),
    ("post.satisfaction.venue", "ด้านสถานที่"),
    ("post.satisfaction.catering", "ด้านอาหารเครื่องดื่ม"),
    ("post.satisfaction.promotion", "ด้านการประชาสัมพันธ์"),
];

/// The three-point scale, in the Google Form's own order and wording.
const SCALE: [&str; 3] = ["ไม่พึงพอใจ", "พึงพอใจ", "พึงพอใจมาก"];

const KEY_COMMENT: &str = "post.comment";
const KEY_NEXT_TOPICS: &str = "post.next_topics";
const KEY_LATENT_SPACE: &str = "post.latent_space_continue";

/// The Google Form asks this once, at the end, as a single choice.
const LATENT_SPACE_OPTIONS: [&str; 4] = [
    "อยากให้จัดต่อ และจะเข้าร่วม",
    "อยากให้จัดต่อ แต่จะดูย้อนหลังเอา",
    "เฉย ๆ",
    "ไม่เคยดู และไม่ทราบว่ามีซีรีส์นี้",
];

#[derive(Clone, Deserialize)]
struct InboxItem {
    kind: String,
    event_name: String,
    event_slug: String,
}

#[derive(Clone, Default, Deserialize)]
struct InboxPage {
    items: Vec<InboxItem>,
}

#[derive(Clone, Deserialize)]
struct MyRegistration {
    name: String,
}

/// One event's block of answers.
#[derive(Clone)]
struct EventBlock {
    slug: String,
    name: String,
    /// One signal per dimension, in `DIMENSIONS` order.
    ratings: [RwSignal<String>; 4],
    comment: RwSignal<String>,
}

#[derive(Clone, PartialEq)]
enum PageState {
    Loading,
    /// Signed in, has at least one event awaiting feedback.
    Ready,
    /// Signed in with nothing outstanding — a success state, not an error.
    NothingToDo,
    Submitting,
    Done(usize),
    Error(String),
}

/// Build the per-event payload.
///
/// An unanswered dimension is omitted rather than sent empty: the Worker drops
/// empty values anyway, and an absent key is honestly "not answered" where an
/// empty string reads as "answered with nothing".
fn answers_for(block: &EventBlock) -> std::collections::HashMap<String, String> {
    let mut answers = std::collections::HashMap::new();
    for (index, (key, _)) in DIMENSIONS.iter().enumerate() {
        let value = block.ratings[index].get();
        let trimmed = value.trim();
        if !trimmed.is_empty() {
            answers.insert((*key).to_string(), trimmed.to_string());
        }
    }
    let comment = block.comment.get();
    let trimmed = comment.trim();
    if !trimmed.is_empty() {
        answers.insert(KEY_COMMENT.to_string(), trimmed.to_string());
    }
    answers
}

#[component]
#[allow(non_snake_case)]
pub fn Feedback() -> impl IntoView {
    let (state, set_state) = signal(PageState::Loading);
    let (blocks, set_blocks) = signal(Vec::<EventBlock>::new());
    let (name, set_name) = signal(String::new());
    let next_topics = RwSignal::new(String::new());
    let latent_space = RwSignal::new(String::new());

    leptos::task::spawn_local(async move {
        // Auth gate on mount — same self-gating pattern as the single-event
        // form, so a survey link from the inbox works for a logged-out person.
        if crate::api::get_me().await.is_err() {
            let login_url = format!("/login?next={}", urlencoding::encode("/feedback"));
            let _ = web_sys::window().map(|w| w.location().set_href(&login_url));
            return;
        }

        // The attendee row already holds a name and the upsert overwrites it,
        // so send back what is on file rather than inventing one.
        if let Ok(rows) =
            crate::api::api_get_json::<Vec<MyRegistration>>("/my-registrations").await
            && let Some(first) = rows.into_iter().find(|r| !r.name.trim().is_empty())
        {
            set_name.set(first.name);
        }

        let page = match crate::api::api_get_json::<InboxPage>("/my-notifications").await {
            Ok(page) => page,
            Err(e) => {
                set_state.set(PageState::Error(e.message));
                return;
            }
        };

        // One block per event. The same event cannot appear twice today, but
        // dedupe rather than rely on that: a re-enqueued survey would otherwise
        // render two blocks that submit over each other.
        let mut seen = Vec::<String>::new();
        let mut built = Vec::new();
        for item in page.items {
            if item.kind != "survey" || seen.contains(&item.event_slug) {
                continue;
            }
            seen.push(item.event_slug.clone());
            built.push(EventBlock {
                slug: item.event_slug,
                name: item.event_name,
                ratings: [
                    RwSignal::new(String::new()),
                    RwSignal::new(String::new()),
                    RwSignal::new(String::new()),
                    RwSignal::new(String::new()),
                ],
                comment: RwSignal::new(String::new()),
            });
        }

        match built.is_empty() {
            true => set_state.set(PageState::NothingToDo),
            false => {
                set_blocks.set(built);
                set_state.set(PageState::Ready);
            }
        }
    });

    let submit = move |_| {
        set_state.set(PageState::Submitting);
        let all = blocks.get();
        let who = name.get();
        leptos::task::spawn_local(async move {
            let mut saved = 0usize;
            let mut failure: Option<String> = None;
            for (index, block) in all.iter().enumerate() {
                let mut answers = answers_for(block);
                // The two closing questions are about the programme, not about
                // one event. Sending them with every block would multiply one
                // opinion by however many events the person attended, so they
                // ride along with the first block only.
                if index == 0 {
                    for (key, value) in [
                        (KEY_NEXT_TOPICS, next_topics.get()),
                        (KEY_LATENT_SPACE, latent_space.get()),
                    ] {
                        let trimmed = value.trim();
                        if !trimmed.is_empty() {
                            answers.insert(key.to_string(), trimmed.to_string());
                        }
                    }
                }
                if answers.is_empty() {
                    continue;
                }
                let body = PostEventRegisterBody {
                    name: who.clone(),
                    consent_given: true,
                    profile_fields: Some(answers),
                    ..Default::default()
                };
                match register_post_event(&block.slug, &body).await {
                    Ok(_) => saved += 1,
                    // Report the first failure but keep going: a person who
                    // answered for three events should not lose two of them
                    // because the third event closed its form mid-submission.
                    Err(e) => {
                        if failure.is_none() {
                            failure = Some(format!("{}: {}", block.name, e.message));
                        }
                    }
                }
            }
            match failure {
                Some(message) if saved == 0 => set_state.set(PageState::Error(message)),
                _ => set_state.set(PageState::Done(saved)),
            }
        });
    };

    view! {
        <div class="container layout-col-center">
            {move || match state.get() {
                PageState::Loading => view! { <p class="card layout-col-center">"กำลังโหลด…"</p> }.into_any(),
                PageState::NothingToDo => view! {
                    <div class="card layout-col-center">
                        <h1>"ไม่มีแบบสอบถามค้างอยู่"</h1>
                        <p>"ขอบคุณครับ — ตอนนี้ไม่มีงานที่รอความเห็นจากคุณ"</p>
                    </div>
                }.into_any(),
                PageState::Error(message) => view! {
                    <div class="card layout-col-center">
                        <h1>"ส่งไม่สำเร็จ"</h1>
                        <p>{message}</p>
                    </div>
                }.into_any(),
                PageState::Done(saved) => view! {
                    <div class="card layout-col-center">
                        <h1>"ขอบคุณครับ"</h1>
                        <p>{format!("บันทึกความเห็นของคุณแล้ว {saved} งาน")}</p>
                    </div>
                }.into_any(),
                PageState::Ready | PageState::Submitting => {
                    let busy = state.get() == PageState::Submitting;
                    view! {
                        <header class="card">
                            <h1>"ขอความเห็นจากงานที่คุณเข้าร่วม"</h1>
                            <p>
                                "ใช้เวลาประมาณ 2 นาที ข้ามข้อไหนก็ได้ "
                                "คำตอบไปที่ทีมงานโดยตรง ไม่เปิดเผยชื่อในรายงาน"
                            </p>
                        </header>

                        <For
                            each=move || blocks.get()
                            key=|block| block.slug.clone()
                            let:block
                        >
                            <section class="card">
                                <h2>{block.name.clone()}</h2>
                                {DIMENSIONS.iter().enumerate().map(|(index, (_, label))| {
                                    let rating = block.ratings[index];
                                    view! {
                                        <fieldset class="dev-profile-field">
                                            <legend class="dev-profile-label">{*label}</legend>
                                            {SCALE.iter().map(|option| {
                                                let value = (*option).to_string();
                                                let selected = value.clone();
                                                view! {
                                                    <label>
                                                        <input
                                                            type="radio"
                                                            prop:checked=move || rating.get() == selected
                                                            on:change=move |_| rating.set(value.clone())
                                                        />
                                                        {*option}
                                                    </label>
                                                }
                                            }).collect_view()}
                                        </fieldset>
                                    }
                                }).collect_view()}
                                <label class="dev-profile-field">
                                    "ข้อเสนอแนะ"
                                    <textarea
                                        class="dev-profile-input"
                                        rows="3"
                                        prop:value=move || block.comment.get()
                                        on:input=move |ev| block.comment.set(event_target_value(&ev))
                                    />
                                </label>
                            </section>
                        </For>

                        <section class="card">
                            <label>
                                "เนื้อหาที่ท่านสนใจหรืออยากให้มีในการจัดงานครั้งต่อไป"
                                <textarea
                                    class="dev-profile-input"
                                    rows="3"
                                    prop:value=move || next_topics.get()
                                    on:input=move |ev| next_topics.set(event_target_value(&ev))
                                />
                            </label>
                            <fieldset>
                                <legend class="dev-profile-label">
                                    "ซีรีส์ Solana in Latent Space (ออนไลน์ Part 1–6) — อยากให้จัดต่อในไตรมาสหน้าไหม?"
                                </legend>
                                {LATENT_SPACE_OPTIONS.iter().map(|option| {
                                    let value = (*option).to_string();
                                    let selected = value.clone();
                                    view! {
                                        <label>
                                            <input
                                                type="radio"
                                                prop:checked=move || latent_space.get() == selected
                                                on:change=move |_| latent_space.set(value.clone())
                                            />
                                            {*option}
                                        </label>
                                    }
                                }).collect_view()}
                            </fieldset>
                        </section>

                        <button class="btn btn-primary" prop:disabled=busy on:click=submit>
                            {move || match busy {
                                true => "กำลังส่ง…",
                                false => "ส่งความเห็น",
                            }}
                        </button>
                    }.into_any()
                }
            }}
        </div>
    }
}
