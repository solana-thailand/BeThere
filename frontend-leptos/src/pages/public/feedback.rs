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
use crate::pages::landing::{AuthState, SiteHeader};

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

/// The dimensions to ask about, by how the person took part.
///
/// DevRel runs two Google Forms, and the online one asks a **single**
/// unlabelled 1–3 satisfaction item where the onsite one asks four named
/// dimensions. The reason is obvious once stated: someone who watched a
/// livestream has no opinion on the room or the coffee, and asking anyway
/// produces a number that looks like data.
///
/// Content and promotion apply to both. Venue and catering are onsite-only.
fn dimensions_for(participation_type: &str) -> &'static [usize] {
    match participation_type {
        "online" => &[0, 3],
        _ => &[0, 1, 2, 3],
    }
}

/// Why an online registrant did not watch.
///
/// **This question is in neither Google Form.** It lives in the covering email
/// as a sentence — *"คำถามที่สำคัญที่สุดสำหรับเรา: อะไรที่ทำให้คุณ 'ไม่ได้ดู'
/// ทั้งที่ลงทะเบียนไว้ — เวลา หัวข้อ ภาษา หรือไม่รู้ว่าไลฟ์แล้ว"* — so the
/// answer, if it came at all, arrived as free text in a comment box. DevRel
/// calls it the most useful thing the survey can find, and the gap it measures
/// is real: 337 online registrations against 7,553 archive views.
///
/// Asked only of `online` attendees, and first, because for someone who did not
/// watch it is the only question they can honestly answer (`.issues/098`).
const KEY_WATCHED: &str = "post.online.watched";
const WATCHED_OPTIONS: [&str; 6] = [
    "ได้ดูสด",
    "ดูย้อนหลัง",
    "ไม่ได้ดู — ติดเวลา",
    "ไม่ได้ดู — หัวข้อไม่ตรงที่สนใจ",
    "ไม่ได้ดู — ภาษา",
    "ไม่ได้ดู — ไม่รู้ว่าไลฟ์แล้ว",
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

/// Only the display name, which the post-event upsert overwrites.
#[derive(Clone, Deserialize)]
struct MyRegistration {
    name: String,
}

/// One rateable session, from `GET /api/my-feedback-events`.
///
/// Not from the notification inbox any more: migration 0038 collapsed the queue
/// to one row per person, which is right for sending and wrong for a form whose
/// unit is the event. The message is per person, the form is per (person,
/// event), and each now reads a source at its own grain (`.issues/102`).
///
/// It also carries the poster and venue, so the page no longer fetches each
/// event's public payload separately.
#[derive(Clone, Default, Deserialize)]
struct FeedbackEvent {
    slug: String,
    event_name: String,
    #[serde(default)]
    event_start_ms: i64,
    #[serde(default)]
    location: String,
    #[serde(default)]
    poster_url: String,
    #[serde(default)]
    nft_image_url: String,
    #[serde(default)]
    participation_type: String,
    #[serde(default)]
    answered: i64,
}

/// One event's block of answers.
#[derive(Clone)]
struct EventBlock {
    slug: String,
    name: String,
    /// Poster, date and venue, to place the session.
    ///
    /// Four months is long enough to forget which session was which, and a
    /// person who cannot place the event either abandons the form or answers
    /// about the wrong one. DevRel hit this with the Google Forms and solved it
    /// by pasting recordings into the mail; the same problem, solved where the
    /// question actually is.
    ///
    /// Plain values, not signals: `/api/my-feedback-events` returns them with
    /// the list, so there is nothing to arrive later.
    event_start_ms: i64,
    location: String,
    image: String,
    participation_type: String,
    /// Online only: whether they watched, and if not what got in the way.
    watched: RwSignal<String>,
    /// One signal per dimension, in `DIMENSIONS` order. Every block carries all
    /// four; `dimensions_for` decides which are asked and which are submitted.
    ratings: [RwSignal<String>; 4],
    comment: RwSignal<String>,
    /// The comment box starts collapsed. The fast path is four taps per event;
    /// a textarea per event that most people leave blank is what makes a short
    /// form look like a long one.
    comment_open: RwSignal<bool>,
    /// Whether the whole question set is showing.
    ///
    /// Only the first block opens by default. 123 of 206 people have exactly
    /// one session and never see the difference; the 29 with four or more are
    /// the regulars, and for them an expanded wall is the difference between a
    /// form and a chore (`.issues/103`).
    open: RwSignal<bool>,
    /// Answered on a previous visit. Distinct from `block_answered`, which only
    /// sees what was typed in this session — the page used to know nothing
    /// about earlier submissions, so it reported `0 จาก 11` to someone who had
    /// already answered one and offered to take the answer again
    /// (`.issues/107`).
    already: bool,
}

/// Has this block been answered at all? Drives the progress line and the tick
/// on a collapsed row.
fn block_answered(block: &EventBlock) -> bool {
    block.already
        || block.ratings.iter().any(|r| !r.get().trim().is_empty())
        || !block.watched.get().trim().is_empty()
        || !block.comment.get().trim().is_empty()
}

#[derive(Clone, PartialEq)]
enum PageState {
    Loading,
    /// Signed in, has at least one event awaiting feedback.
    Ready,
    /// Signed in with nothing outstanding — a success state, not an error.
    NothingToDo,
    /// Signed in, but on a wallet-only session.
    ///
    /// The survey is addressed to a verified email, so the Worker answers 403.
    /// That used to land in `Error`, whose heading reads "ส่งไม่สำเร็จ" — wrong
    /// twice over: nothing was submitted, and the message was an English
    /// instruction with no button to act on. `/login` offers wallet and Google
    /// side by side, so anyone arriving from the survey link who picks wallet
    /// first hit a dead end (`.issues/106`).
    NeedsGoogle,
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
    for index in dimensions_for(&block.participation_type) {
        let value = block.ratings[*index].get();
        let trimmed = value.trim();
        if !trimmed.is_empty() {
            answers.insert(DIMENSIONS[*index].0.to_string(), trimmed.to_string());
        }
    }
    let watched = block.watched.get();
    let trimmed = watched.trim();
    if !trimmed.is_empty() {
        answers.insert(KEY_WATCHED.to_string(), trimmed.to_string());
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
    // Every page needs a way onward. This one had none — the reader arrived from
    // an email and the only exit was the browser's back button (`.issues/107`).
    // Signed in by construction: the page redirects to /login otherwise.
    let auth_state = RwSignal::new(AuthState::Checking);
    let (user_role, set_user_role) = signal(String::new());
    let next_topics = RwSignal::new(String::new());
    let latent_space = RwSignal::new(String::new());

    leptos::task::spawn_local(async move {
        // Auth gate on mount — same self-gating pattern as the single-event
        // form, so a survey link from the inbox works for a logged-out person.
        match crate::api::get_me().await {
            Ok(me) => {
                set_user_role.set(me.role.clone());
                auth_state.set(AuthState::SignedIn(me.email));
            }
            Err(_) => {
                let login_url = format!("/login?next={}", urlencoding::encode("/feedback"));
                let _ = web_sys::window().map(|w| w.location().set_href(&login_url));
                return;
            }
        }

        // The attendee row already holds a name and the upsert overwrites it,
        // so send back what is on file rather than inventing one.
        if let Ok(rows) = crate::api::api_get_json::<Vec<MyRegistration>>("/my-registrations").await
            && let Some(first) = rows.into_iter().find(|r| !r.name.trim().is_empty())
        {
            set_name.set(first.name);
        }

        let events =
            match crate::api::api_get_json::<Vec<FeedbackEvent>>("/my-feedback-events").await {
                Ok(events) => events,
                // A wallet-only session: the survey is addressed to a verified
                // email, so the Worker answers 403. Its own state, because
                // `Error` reads "ส่งไม่สำเร็จ" — nothing was submitted — and
                // offered no way out (`.issues/106`).
                Err(e) if e.status == 403 => {
                    set_state.set(PageState::NeedsGoogle);
                    return;
                }
                Err(e) => {
                    set_state.set(PageState::Error(e.message));
                    return;
                }
            };

        let built: Vec<EventBlock> = events
            .into_iter()
            .map(|e| EventBlock {
                slug: e.slug,
                name: e.event_name,
                event_start_ms: e.event_start_ms,
                location: e.location,
                // Poster first, badge second — the fallback `event_hero`
                // documents. The generic badge SVG is the same image for every
                // event, so it identifies nothing and is better left out.
                image: match (
                    e.poster_url.is_empty(),
                    e.nft_image_url.contains("badge-hd.svg"),
                ) {
                    (false, _) => e.poster_url,
                    (true, false) => e.nft_image_url,
                    _ => String::new(),
                },
                participation_type: e.participation_type,
                already: e.answered != 0,
                watched: RwSignal::new(String::new()),
                comment_open: RwSignal::new(false),
                open: RwSignal::new(false),
                ratings: [
                    RwSignal::new(String::new()),
                    RwSignal::new(String::new()),
                    RwSignal::new(String::new()),
                    RwSignal::new(String::new()),
                ],
                comment: RwSignal::new(String::new()),
            })
            .collect();

        match built.iter().find(|b| !b.already).or_else(|| built.first()) {
            None => set_state.set(PageState::NothingToDo),
            Some(first) => {
                // The list arrives most-recent-first, so the one block that
                // opens by default is the session they remember best. If they
                // answer only that one, it is the most reliable answer they
                // could have given.
                first.open.set(true);
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
        <div class="container fb-page">
            <SiteHeader auth_state=auth_state.read_only() user_role=user_role />
            {move || match state.get() {
                PageState::Loading => view! { <p class="card layout-col-center">"กำลังโหลด…"</p> }.into_any(),
                PageState::NothingToDo => view! {
                    <div class="card fb-notice">
                        <h1>"ไม่มีแบบสอบถามค้างอยู่"</h1>
                        <p>"ขอบคุณครับ — ตอนนี้ไม่มีงานที่รอความเห็นจากคุณ"</p>
                        <a class="btn btn-primary" href="/">"กลับหน้าหลัก"</a>
                    </div>
                }.into_any(),
                PageState::NeedsGoogle => view! {
                    <div class="card fb-notice">
                        <h1>"ต้องเข้าสู่ระบบด้วย Google"</h1>
                        <p>
                            "แบบสอบถามผูกกับอีเมลที่คุณใช้ลงทะเบียนงาน "
                            "การเข้าสู่ระบบด้วยกระเป๋าเงินจึงยังไม่พอ"
                        </p>
                        <a class="btn btn-primary" href="/login?next=/feedback">
                            "เข้าสู่ระบบด้วย Google"
                        </a>
                    </div>
                }.into_any(),
                PageState::Error(message) => view! {
                    <div class="card fb-notice">
                        <h1>"ส่งไม่สำเร็จ"</h1>
                        <p>{message}</p>
                    </div>
                }.into_any(),
                // Somewhere to go next. This used to be a full stop: a sentence
                // and no button, on a page most people reach with more sessions
                // still unanswered (`.issues/107`).
                PageState::Done(saved) => view! {
                    <div class="card fb-notice">
                        <h1>"ขอบคุณครับ"</h1>
                        <p>{format!("บันทึกความเห็นของคุณแล้ว {saved} งาน")}</p>
                        <div class="fb-done-actions">
                            <a class="btn btn-primary" href="/feedback">
                                "ให้ความเห็นงานอื่นต่อ"
                            </a>
                            <a class="btn btn-outline" href="/">"กลับหน้าหลัก"</a>
                        </div>
                    </div>
                }.into_any(),
                PageState::Ready | PageState::Submitting => {
                    let busy = state.get() == PageState::Submitting;
                    // Defined out here: the `view!` macro cannot parse a
                    // turbofish inside an attribute value.
                    let newest = move || blocks.get().into_iter().next();
                    let older = move || {
                        let rest: Vec<EventBlock> = blocks.get().into_iter().skip(1).collect();
                        rest
                    };
                    view! {
                        <header class="card">
                            <h1>"ขอความเห็นจากงานที่คุณเข้าร่วม"</h1>
                            <p>
                                "ใช้เวลาประมาณ 2 นาที ข้ามข้อไหนก็ได้ "
                                "คำตอบไปที่ทีมงานโดยตรง ไม่เปิดเผยชื่อในรายงาน"
                            </p>
                            // Only shown to people with more than one session,
                            // which is 83 of 206. For the other 123 a counter
                            // reading "1 จาก 1" is noise.
                            <Show when=move || { blocks.get().len() > 1 } fallback=|| ()>
                                <p class="fb-progress">
                                    {move || {
                                        let all = blocks.get();
                                        let done = all.iter().filter(|b| block_answered(b)).count();
                                        format!("ตอบแล้ว {done} จาก {} งาน — ส่งได้เลยไม่ต้องครบ", all.len())
                                    }}
                                </p>
                            </Show>
                        </header>

                        // The most recent session, open. Four taps, and the
                        // person is already finished if they want to be.
                        {move || match newest() {
                            None => view! { <div></div> }.into_any(),
                            Some(first) => view! { <EventQuestionBlock block=first /> }.into_any(),
                        }}

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
                            <fieldset class="fb-options">
                                <legend class="dev-profile-label">
                                    "ซีรีส์ Solana in Latent Space (ออนไลน์ Part 1–6) — อยากให้จัดต่อในไตรมาสหน้าไหม?"
                                </legend>
                                {LATENT_SPACE_OPTIONS.iter().map(|option| {
                                    let value = (*option).to_string();
                                    let selected = value.clone();
                                    let set_to = value.clone();
                                    view! {
                                        <button
                                            type="button"
                                            class=move || match latent_space.get() == selected {
                                                true => "fb-option is-selected",
                                                false => "fb-option",
                                            }
                                            aria-pressed=move || (latent_space.get() == value).to_string()
                                            on:click=move |_| latent_space.set(set_to.clone())
                                        >
                                            {*option}
                                        </button>
                                    }
                                }).collect_view()}
                            </fieldset>
                        </section>

                        // Everything older, collapsed. See `.issues/103`.
                        <For
                            each=older
                            key=|block| block.slug.clone()
                            let:block
                        >
                            <EventQuestionBlock block=block />
                        </For>


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

/// One session's question set, collapsed unless it is the one being answered.
#[component]
fn EventQuestionBlock(block: EventBlock) -> impl IntoView {
    // Signals are `Copy`; the rest of the block is not. Clone once per closure
    // that needs the data rather than threading references through the view.
    let open = block.open;
    let answered = block.clone();
    view! {
                        <section class=move || match block.open.get() {
                            true => "card fb-event",
                            // Collapsed blocks are a list, not a stack of cards:
                            // eleven cards is a wall whatever is inside them.
                            false => "card fb-event is-collapsed",
                        }>
                            // Poster, date and venue before the questions —
                            // the point of the block is that the reader
                            // recognises the event before rating it.
                            //
                            // A button, not a link to the event page. Nothing
                            // on this form is persisted until submit, so a
                            // navigation away from a half-filled block throws
                            // the answers away — the header was one tap from
                            // losing someone's work. Recognition is what the
                            // poster, name, date and venue are for; the event
                            // page adds nothing worth that risk.
                            <button
                                type="button"
                                class="fb-event-head"
                                aria-expanded=move || open.get().to_string()
                                on:click=move |_| open.update(|v| *v = !*v)
                            >
                                {match block.image.is_empty() {
                                    // No frame at all rather than an empty
                                    // one: six events have no poster, and a
                                    // blank box identifies nothing.
                                    true => view! { <div></div> }.into_any(),
                                    false => view! {
                                        <img class="fb-event-poster" src=block.image.clone() alt="Event poster" />
                                    }.into_any(),
                                }}
                                <div class="fb-event-meta">
                                    <h2>{block.name.clone()}</h2>
                                    <p class="subtitle">
                                        {
                                            let when = crate::utils::format_event_day(block.event_start_ms);
                                            [when, block.location.clone()]
                                                .into_iter()
                                                .filter(|part| !part.is_empty())
                                                .collect::<Vec<_>>()
                                                .join(" · ")
                                        }
                                    </p>
                                </div>
                                // A tick on a collapsed row is the only way to
                                // tell answered from skipped without opening it.
                                // "ตอบแล้ว" is a stronger claim than the tick and
                                // is reserved for an answer that is actually
                                // stored — otherwise someone who typed and left
                                // would be told their answer was saved.
                                {move || match (!open.get(), answered.already, block_answered(&answered)) {
                                    (true, true, _) => view! { <span class="fb-badge">"ตอบแล้ว"</span> }.into_any(),
                                    (true, false, true) => view! { <span class="fb-tick">"✓"</span> }.into_any(),
                                    (true, false, false) => view! { <span class="fb-chevron">"+"</span> }.into_any(),
                                    _ => view! { <div></div> }.into_any(),
                                }}
                            </button>

                            <Show when=move || open.get() fallback=|| ()>
                            // Online only, and first: for someone who did
                            // not watch, this is the only question they can
                            // answer honestly, and it is the one DevRel
                            // most wants answered (.issues/098).
                            {
                                let watched = block.watched;
                                match block.participation_type == "online" {
                                    false => view! { <div></div> }.into_any(),
                                    true => view! {
                                        <fieldset class="fb-options">
                                            <legend class="dev-profile-label">
                                                "คุณได้ดูงานนี้ไหม"
                                            </legend>
                                            {WATCHED_OPTIONS.iter().map(|option| {
                                                let value = (*option).to_string();
                                                let selected = value.clone();
                                                let set_to = value.clone();
                                                view! {
                                                    <button
                                                        type="button"
                                                        class=move || match watched.get() == selected {
                                                            true => "fb-option is-selected",
                                                            false => "fb-option",
                                                        }
                                                        aria-pressed=move || (watched.get() == value).to_string()
                                                        on:click=move |_| watched.set(set_to.clone())
                                                    >
                                                        {*option}
                                                    </button>
                                                }
                                            }).collect_view()}
                                        </fieldset>
                                    }.into_any(),
                                }
                            }

                            {dimensions_for(&block.participation_type).iter().map(|index| {
                                let index = *index;
                                let label = DIMENSIONS[index].1;
                                let rating = block.ratings[index];
                                view! {
                                    // One row per dimension: label left, three
                                    // choices right. Four stacked fieldsets of
                                    // radios read as a wall; the whole event is
                                    // four taps.
                                    <div class="fb-row">
                                        <span class="fb-row-label">{label}</span>
                                        <div class="fb-scale" role="group" aria-label=label>
                                            {SCALE.iter().map(|option| {
                                                let value = (*option).to_string();
                                                let selected = value.clone();
                                                let set_to = value.clone();
                                                view! {
                                                    <button
                                                        type="button"
                                                        class=move || match rating.get() == selected {
                                                            true => "fb-choice is-selected",
                                                            false => "fb-choice",
                                                        }
                                                        aria-pressed=move || (rating.get() == value).to_string()
                                                        on:click=move |_| rating.set(set_to.clone())
                                                    >
                                                        {*option}
                                                    </button>
                                                }
                                            }).collect_view()}
                                        </div>
                                    </div>
                                }
                            }).collect_view()}

                            {
                                let open = block.comment_open;
                                let comment = block.comment;
                                move || match open.get() {
                                    false => view! {
                                        <button
                                            type="button"
                                            class="fb-add-comment"
                                            on:click=move |_| open.set(true)
                                        >
                                            "+ เพิ่มข้อเสนอแนะ"
                                        </button>
                                    }.into_any(),
                                    true => view! {
                                        <textarea
                                            class="dev-profile-input"
                                            rows="3"
                                            placeholder="ข้อเสนอแนะ (ไม่บังคับ)"
                                            prop:value=move || comment.get()
                                            on:input=move |ev| comment.set(event_target_value(&ev))
                                        />
                                    }.into_any(),
                                }
                            }
                            </Show>
                        </section>
    }
}
