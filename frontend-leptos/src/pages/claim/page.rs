//! Claim page component — public route at `/claim/:token`.

use leptos::prelude::*;
use leptos_meta::Title;
use leptos_router::hooks::use_params;

use crate::api::{self, AdventureStatusType, QuizStatus};
use crate::i18n::{t, t_string, use_i18n};
use crate::icons::{Icon, IconName, wallet_icon_name};
use crate::utils::escape_html;

use super::helpers::*;
use super::interop::*;
use super::quiz_views::*;
use super::state::*;
use super::stepper::*;
use super::widgets::*;

// ---------------------------------------------------------------------------
// Claim page component
// ---------------------------------------------------------------------------

// Claim page component — public route at `/claim/:token`.
//
// Attendees scan their claim QR code (or follow the claim URL) to land here.
/// The page looks up their check-in record and allows them to mint a
/// compressed NFT badge to their Solana wallet.
#[component]
pub fn Claim() -> impl IntoView {
    let params = use_params::<ClaimParams>();

    // Reactive state
    let (state, set_state) = signal(ClaimState::Loading);
    let (wallet_input, set_wallet_input) = signal(String::new());

    // Quiz state — selected answer text per question (question_id → option text)
    let (quiz_answers, set_quiz_answers): (ReadSignal<QuizAnswers>, WriteSignal<QuizAnswers>) =
        signal(QuizAnswers::new());

    // Dynamic event config (fetched from backend, replaces hardcoded values)
    let (evt_name, set_evt_name) = signal(String::new());
    let (evt_tagline, set_evt_tagline) = signal(String::new());
    let (evt_link, set_evt_link) = signal(String::new());
    let (evt_start, set_evt_start) = signal(0i64);
    let (evt_end, set_evt_end) = signal(0i64);

    // Share feedback

    // Claim counter (fetched from backend on initial lookup)
    let (_total_checked_in, set_total_checked_in) = signal(0usize);
    let (_total_claimed, set_total_claimed) = signal(0usize);

    // Deposit info (persisted across state transitions)
    let (deposit_api_id, set_deposit_api_id) = signal(String::new());
    let (deposit_event_id, set_deposit_event_id) = signal(String::new());
    let (deposit_enabled, set_deposit_enabled) = signal(false);
    let (_deposit_amount_usdc, set_deposit_amount_usdc) = signal(0u64);
    let (_deposit_amount_thb, set_deposit_amount_thb) = signal(0u64);

    // Wallet adapter state — detected wallets and connected wallet info
    let (detected_wallets, set_detected_wallets) = signal(Vec::<String>::new());
    let (connected_wallet, set_connected_wallet) = signal(None::<(String, String)>); // (wallet_name, public_key)
    // When the attendee has a verified linked wallet, the claim defaults to a
    // one-tap "mint to my linked wallet" path (resolved server-side) and hides the
    // connect options until the user opts to use a different wallet.
    let (change_wallet, set_change_wallet) = signal(false);
    let (has_linked_wallet, set_has_linked_wallet) = signal(false);

    // Detect installed wallets on mount (poll with delay for late injection)
    {
        let set_dw = set_detected_wallets;
        leptos::task::spawn_local(async move {
            let mut wallets = get_detected_wallets_js();
            if wallets.is_empty() {
                for _ in 0..10 {
                    gloo_timers::future::TimeoutFuture::new(300).await;
                    wallets = get_detected_wallets_js();
                    if !wallets.is_empty() {
                        break;
                    }
                }
            }
            log::info!("[claim] detected wallets: {:?}", wallets);
            set_dw.set(wallets);
        });
    }

    // Extract token from URL params and fetch claim info on mount
    Effect::new(move |_| {
        let token = match params.get() {
            Ok(p) => p.token.unwrap_or_default(),
            Err(_) => {
                set_state.set(ClaimState::NotFound(ClaimNotFound::MissingToken));
                return;
            }
        };

        if token.is_empty() {
            set_state.set(ClaimState::NotFound(ClaimNotFound::MissingToken));
            return;
        }

        // Fetch claim info
        leptos::task::spawn_local(async move {
            match api::get_claim(&token).await {
                Ok(data) => {
                    // Set dynamic event config from backend
                    set_evt_name.set(data.event.event_name.clone());
                    set_evt_tagline.set(data.event.event_tagline.clone());
                    set_evt_link.set(data.event.event_link.clone());
                    set_evt_start.set(data.event.event_start_ms);
                    set_evt_end.set(data.event.event_end_ms);
                    set_total_checked_in.set(data.total_checked_in);
                    set_total_claimed.set(data.total_claimed);

                    // Store deposit info for use across state transitions
                    set_deposit_api_id.set(data.api_id.clone());
                    set_deposit_event_id.set(data.event_id.clone());
                    set_deposit_enabled.set(data.deposit_enabled);
                    set_deposit_amount_usdc.set(data.deposit_amount_usdc);
                    set_deposit_amount_thb.set(data.deposit_amount_thb);

                    // Pre-fill the wallet field with the locked per-event address (the
                    // full pre-registered wallet is public and server-enforced). A
                    // linked profile wallet is NOT pre-filled here — its full address
                    // never reaches the client; the one-tap path mints to it server-side.
                    if let Some(w) = data.locked_wallet.clone().filter(|w| !w.is_empty()) {
                        set_wallet_input.set(w);
                    }
                    set_has_linked_wallet.set(
                        data.linked_wallet_display
                            .as_deref()
                            .is_some_and(|w| !w.is_empty()),
                    );

                    if data.claimed {
                        set_state.set(ClaimState::AlreadyClaimed(data));
                    } else if !data.nft_available {
                        set_state.set(ClaimState::NftComingSoon(data));
                    } else if matches!(
                        data.quiz_status,
                        QuizStatus::NotStarted | QuizStatus::InProgress
                    ) {
                        // Quiz required — fetch questions, then route to Quiz state
                        let claim_data = data.clone();
                        leptos::task::spawn_local(async move {
                            match api::get_quiz(Some(&claim_data.event_id)).await {
                                Ok(quiz_data) if quiz_data.configured => {
                                    set_state.set(ClaimState::Quiz(claim_data, quiz_data));
                                }
                                Ok(_) => {
                                    // Quiz enabled but not configured — organizer hasn't set it up yet.
                                    // Show a clear message and block claiming.
                                    log::warn!(
                                        "[claim] quiz status={:?} but quiz not configured, showing quiz pending",
                                        claim_data.quiz_status
                                    );
                                    set_state.set(ClaimState::NftComingSoon(claim_data));
                                }
                                Err(e) => {
                                    log::error!("[claim] failed to fetch quiz: {e}");
                                    // Can't verify quiz status — show coming soon instead of letting claim through
                                    set_state.set(ClaimState::NftComingSoon(claim_data));
                                }
                            }
                        });
                    } else {
                        // (wallet pre-fill already applied above, before branching)
                        // Check adventure status — if required and not passed, show adventure gate
                        let claim_data_for_adventure = data.clone();
                        let token_for_adventure = token.clone();
                        leptos::task::spawn_local(async move {
                            match api::get_adventure_status(
                                &token_for_adventure,
                                Some(&claim_data_for_adventure.event_id),
                            )
                            .await
                            {
                                Ok(status_data) => match status_data.status {
                                    AdventureStatusType::NotRequired => {
                                        set_state.set(ClaimState::Ready(claim_data_for_adventure));
                                    }
                                    AdventureStatusType::Passed => {
                                        set_state.set(ClaimState::Ready(claim_data_for_adventure));
                                    }
                                    AdventureStatusType::NotStarted
                                    | AdventureStatusType::InProgress => {
                                        log::info!(
                                            "[claim] adventure required but not passed, showing adventure gate"
                                        );
                                        set_state.set(ClaimState::Adventure(
                                            claim_data_for_adventure,
                                            status_data.status,
                                        ));
                                    }
                                },
                                Err(e) => {
                                    log::warn!(
                                        "[claim] failed to check adventure status: {e}, proceeding to Ready"
                                    );
                                    // Fallback to Ready so attendee isn't stuck
                                    set_state.set(ClaimState::Ready(claim_data_for_adventure));
                                }
                            }
                        });
                    }
                }
                Err(e) => {
                    log::warn!("[claim] lookup failed for token {token}: {e}");
                    set_state.set(ClaimState::NotFound(ClaimNotFound::LookupFailed(
                        e.to_string(),
                    )));
                }
            }
        });
    });

    // Handle "Claim NFT" button click
    let handle_claim = move |_| {
        // The one-tap linked path mints to the attendee's verified profile wallet,
        // resolved server-side by email — no client address is sent. Any other case
        // (a per-event lock, a connected wallet, or "use a different wallet") sends
        // the explicit address in the input.
        let use_linked =
            has_linked_wallet.get() && !change_wallet.get() && connected_wallet.get().is_none();
        let wallet = wallet_input.get().trim().to_string();
        let token = match params.get() {
            Ok(p) => p.token.unwrap_or_default(),
            Err(_) => return,
        };

        // Basic client-side validation for the explicit-wallet path only.
        if !use_linked {
            let wallet_len = wallet.len();
            if wallet.is_empty() || !(32..=44).contains(&wallet_len) {
                return;
            }
        }

        // Transition to minting state
        let current_data = match state.get() {
            ClaimState::Ready(d) | ClaimState::MintError(d, _) => d,
            _ => return,
        };
        set_state.set(ClaimState::Minting(current_data.clone()));

        let current_data_clone = current_data.clone();
        leptos::task::spawn_local(async move {
            let start = js_sys::Date::now();
            let arg = if use_linked {
                None
            } else {
                Some(wallet.as_str())
            };
            let result = super::mint_retry::post_claim_until_settled(&token, arg, use_linked).await;
            // Ensure spinner displays for at least 1.5s for smooth UX
            let elapsed = js_sys::Date::now() - start;
            if elapsed < 1500.0 {
                let wait = (1500.0 - elapsed) as u32;
                gloo_timers::future::TimeoutFuture::new(wait).await;
            }
            match result {
                Ok(mint_data) => {
                    log::info!(
                        "[claim] minted nft: asset_id={} sig={}",
                        mint_data.asset_id,
                        mint_data.signature
                    );
                    set_state.set(ClaimState::Success(mint_data));
                    // Increment claim counter for display
                    set_total_claimed.update(|n| *n += 1);
                    // Launch confetti celebration!
                    launch_confetti();
                }
                Err(failure) => {
                    set_state.set(ClaimState::MintError(current_data_clone, failure));
                }
            }
        });
    };

    // One-tap paste from clipboard — big mobile UX win
    let handle_paste = move |_| {
        let set_w = set_wallet_input;
        leptos::task::spawn_local(async move {
            let promise = read_clipboard_text_js();
            if let Ok(val) = js_sys::futures::JsFuture::from(promise).await
                && let Some(text) = val.as_string()
            {
                let trimmed: String = text.trim().to_string();
                if !trimmed.is_empty() {
                    set_w.set(trimmed);
                }
            }
        });
    };

    // Clone signal setters for use in nested reactive closures
    let set_w_for_connect = set_wallet_input;
    let set_cw_for_connect = set_connected_wallet;

    let i18n = use_i18n();
    view! {
        <div class="center-page">
            <Title text=crate::locale::tr(|l| crate::i18n::td_string!(l, claim.page_title)) />
            <div class="container claim-container">
                // Brand header
                <div class="brand-logo">"BeThere"</div>
                <div class="brand-logo-sub">{crate::locale::tr(|l| crate::i18n::td_string!(l, claim.proof_of_attendance))}</div>

                // Title
                <h1 class="claim-title">{crate::locale::tr(|l| crate::i18n::td_string!(l, claim.title))}</h1>

                <p class="claim-subtitle">
                    {move || evt_name.get()}
                </p>
                <p class="claim-tagline">
                    {move || evt_tagline.get()}
                </p>
                <p class="claim-event-link">
                    <a href=move || evt_link.get() target="_blank" rel="noopener noreferrer">
                        {move || evt_link.get()}
                    </a>
                </p>

                // Live session timer (reactive — waits for event config from backend)
                {move || {
                    let start = evt_start.get();
                    let end = evt_end.get();
                    if start > 0 && end > 0 {
                        view! { <SessionTimer start_ms=start end_ms=end /> }.into_any()
                    } else {
                        view! { <div class="session-timer"></div> }.into_any()
                    }
                }}

                // Progress stepper — shows claim flow progress
                {move || {
                    let (current, total) = claim_step(&state.get());
                    // Only show when flow has started (not loading/not found)
                    if current > 0 {
                        // Determine if quiz step is needed for this flow
                        let show_quiz = matches!(
                            state.get(),
                            ClaimState::Quiz(_, _)
                                | ClaimState::QuizSubmitted(_, _, _)
                        );
                        view! {
                            <ClaimStepper current=current total=total show_quiz=show_quiz />
                        }.into_any()
                    } else {
                        view! { <div></div> }.into_any()
                    }
                }}

                // State-dependent rendering
                {move || {
                    match state.get() {
                        // ---- Loading ----
                        ClaimState::Loading => {
                            view! {
                                <div class="claim-state-full">
                                    // Shimmer: welcome card (avatar + 2 text lines)
                                    <div class="shimmer-card claim-shimmer-row">
                                        <div class="shimmer shimmer-avatar"></div>
                                        <div class="claim-shimmer-col">
                                            <div class="shimmer shimmer-line claim-shimmer-line-60"></div>
                                            <div class="shimmer shimmer-line-sm claim-shimmer-line-sm-40"></div>
                                        </div>
                                    </div>

                                    // Shimmer: NFT preview card (square + 2 text lines)
                                    <div class="shimmer-card claim-shimmer-row">
                                        <div class="shimmer claim-shimmer-nft"></div>
                                        <div class="claim-shimmer-col">
                                            <div class="shimmer shimmer-line claim-shimmer-line-75"></div>
                                            <div class="shimmer shimmer-line-sm claim-shimmer-line-sm-50"></div>
                                        </div>
                                    </div>

                                    // Shimmer: wallet input card (label + input bar + hint)
                                    <div class="shimmer-card u-mb-sm">
                                        <div class="shimmer shimmer-line-sm u-mb-sm claim-shimmer-line-sm-40"></div>
                                        <div class="shimmer claim-shimmer-input"></div>
                                        <div class="shimmer shimmer-line-sm claim-shimmer-line-sm-55"></div>
                                    </div>

                                    // Shimmer: claim button
                                    <div class="shimmer shimmer-btn claim-shimmer-btn-full"></div>
                                </div>
                            }
                                .into_any()
                        }

                        // ---- Not Found / Error ----
                        ClaimState::NotFound(reason) => {
                            let msg = match reason {
                                ClaimNotFound::MissingToken => crate::locale::tr(|l| crate::i18n::td_string!(l, claim.invalid_link)).into_any(),
                                ClaimNotFound::LookupFailed(e) => {
                                    let error = escape_html(&e);
                                    t!(i18n, claim.lookup_failed, error).into_any()
                                }
                            };
                            view! {
                                <div class="claim-error">
                                    <h2>{crate::locale::tr(|l| crate::i18n::td_string!(l, claim.not_found_title))}</h2>
                                    <div class="result-details">
                                        <p>{msg}</p>
                                    </div>
                                    <a href="/" class="btn btn-outline claim-retry-btn">
                                        {crate::locale::tr(|l| crate::i18n::td_string!(l, claim.go_home))}
                                    </a>
                                </div>
                            }
                                .into_any()
                        }

                        // ---- NFT Coming Soon / Quiz Pending ----
                        ClaimState::NftComingSoon(data) => {
                            let checked_in_display = checked_in_label(&data.checked_in_at, &data.participation_type);
                            // Differentiate: quiz not configured vs NFT not available
                            let (card_title, card_msg, card_detail) = if data.nft_available {
                                // NFT tech is ready but quiz blocks claiming
                                (
                                    crate::locale::tr(|l| crate::i18n::td_string!(l, claim.soon.quiz_title)).into_any(),
                                    crate::locale::tr(|l| crate::i18n::td_string!(l, claim.soon.quiz_msg)).into_any(),
                                    crate::locale::tr(|l| crate::i18n::td_string!(l, claim.soon.quiz_detail)).into_any(),
                                )
                            } else {
                                (
                                    crate::locale::tr(|l| crate::i18n::td_string!(l, claim.soon.nft_title)).into_any(),
                                    crate::locale::tr(|l| crate::i18n::td_string!(l, claim.soon.nft_msg)).into_any(),
                                    crate::locale::tr(|l| crate::i18n::td_string!(l, claim.soon.nft_detail)).into_any(),
                                )
                            };
                            // Before check-in, one line and the badge to unlock; the
                            // pending card and wallet hint only once checked in
                            // (.issues/173 C8).
                            let awaiting = awaiting_check_in(&data.checked_in_at, &data.participation_type);
                            if awaiting {
                                return view! {
                                    <div class="claim-state-full">
                                        <div class="claim-welcome-card">
                                            <ParticipantAvatar name=data.name.clone() />
                                            <h3>{
                                                let name = escape_html(&data.name);
                                                t!(i18n, claim.welcome, name)
                                            }</h3>
                                            <p class="checked-in-label">
                                                {crate::locale::tr(|l| crate::i18n::td_string!(l, claim.check_in_to_unlock))}
                                            </p>
                                        </div>
                                        <NftBadgePreview />
                                    </div>
                                }
                                    .into_any();
                            }
                            view! {
                                <div class="claim-state-full">
                                    // Attendee welcome
                                    <div class="claim-welcome-card">
                                        <ParticipantAvatar name=data.name.clone() />
                                        <h3>{
                                            let name = escape_html(&data.name);
                                            t!(i18n, claim.welcome, name)
                                        }</h3>
                                        <p class="checked-in-label">{checked_in_display}</p>
                                    </div>

                                    // Status card (the pending state carries the badge
                                    // message; the separate preview card is only shown
                                    // before check-in) — quiz pending or NFT coming soon
                                    <div class="claim-nft-soon-card">
                                        {move || view! {
                                            <crate::components::StatusBadge
                                                tone=crate::components::StatusTone::Pending
                                                label=t_string!(i18n, claim.pending)
                                            />
                                        }}
                                        <h3>{card_title}</h3>
                                        <p>{card_msg}</p>
                                        <div class="nft-description">
                                            {card_detail}
                                        </div>
                                    </div>

                                    // Compact wallet hint
                                    <p class="claim-bookmark-hint">
                                        {t!(
                                            i18n,
                                            claim.bookmark_hint,
                                            <wallet> = |children: ChildrenFn| view! {
                                                <a href="https://phantom.app/" target="_blank" rel="noopener noreferrer">{children()}</a>
                                            }
                                        )}
                                    </p>
                                </div>
                            }
                                .into_any()
                        }

                        // ---- Quiz required ----
                        ClaimState::Quiz(claim_data, quiz_data) => {
                            view! {
                                <QuizView
                                    claim_data=claim_data
                                    quiz_data=quiz_data
                                    quiz_answers=quiz_answers
                                    set_quiz_answers=set_quiz_answers
                                    set_state=set_state
                                />
                            }
                                .into_any()
                        }

                        // ---- Quiz submitted — results ----
                        ClaimState::QuizSubmitted(claim_data, quiz_data, submit_result) => {
                            view! {
                                <QuizSubmittedView
                                    claim_data=claim_data
                                    quiz_data=quiz_data
                                    submit_result=submit_result
                                    set_quiz_answers=set_quiz_answers
                                    wallet_input=wallet_input
                                    set_wallet_input=set_wallet_input
                                    set_state=set_state
                                />
                            }
                                .into_any()
                        }

                        // ---- Ready: show wallet input ----
                        ClaimState::Ready(data) => {
                            let checked_in_display = checked_in_label(&data.checked_in_at, &data.participation_type);
                            let locked_wallet = data.locked_wallet.clone();
                            let linked_wallet_display = data
                                .linked_wallet_display
                                .clone()
                                .filter(|w| !w.is_empty());
                            let has_suggested_wallet = linked_wallet_display.is_some();
                            view! {
                                <div class="claim-state-full">
                                    // Attendee welcome
                                    <div class="claim-welcome-card">
                                        <ParticipantAvatar name=data.name.clone() />
                                        <h3>{
                                            let name = escape_html(&data.name);
                                            t!(i18n, claim.welcome, name)
                                        }</h3>
                                        <p class="checked-in-label">{checked_in_display}</p>
                                    </div>

                                    // NFT badge preview
                                    <NftBadgePreview />

                                    // Wallet input — wallet adapter + manual fallback
                                    <div class="card">
                                        <label class="claim-wallet-label">
                                            {crate::locale::tr(|l| crate::i18n::td_string!(l, claim.wallet.address_label))}
                                        </label>

                                        // Locked wallet pill + wallet adapter section (single reactive closure)
                                        {move || {
                                            // Locked wallet pill badge — shown when pre-registered wallet exists
                                            let is_locked = matches!(&locked_wallet, Some(w) if !w.is_empty());
                                            if is_locked {
                                                let w = locked_wallet.as_ref().unwrap();
                                                let truncated = if w.len() > 12 {
                                                    format!("{}...{}", &w[..4], &w[w.len()-4..])
                                                } else {
                                                    w.clone()
                                                };
                                                view! {
                                                    <div class="claim-wallet-locked">
                                                        <svg viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round">
                                                            <rect x="3" y="7" width="10" height="7" rx="1.5"></rect>
                                                            <path d="M5 7V5a3 3 0 0 1 6 0v2"></path>
                                                        </svg>
                                                        <span class="locked-wallet-addr">{truncated}</span>
                                                    </div>
                                                }.into_any()
                                            } else if has_suggested_wallet
                                                && connected_wallet.get().is_none()
                                                && !change_wallet.get()
                                            {
                                                // Profile-linked wallet — the primary one-tap path.
                                                // The connect/paste options are tucked behind
                                                // "Use a different wallet". The address is already
                                                // masked server-side (full address never sent here).
                                                let truncated = linked_wallet_display
                                                    .clone()
                                                    .unwrap_or_default();
                                                view! {
                                                    <div class="wallet-connected-bar">
                                                        <span class="wallet-icon-lg">
                                                            <svg viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round">
                                                                <rect x="2" y="4" width="12" height="9" rx="1.5"></rect>
                                                                <path d="M11 8.5h.01"></path>
                                                            </svg>
                                                        </span>
                                                        <div class="wallet-info-left">
                                                            <div class="wallet-label">{crate::locale::tr(|l| crate::i18n::td_string!(l, claim.wallet.linked_label))}</div>
                                                            <div class="wallet-address-bold">{truncated}</div>
                                                        </div>
                                                        <span class="badge badge-success u-ml-auto"><Icon icon=IconName::Check class="icon-sm icon-success" />" "{crate::locale::tr(|l| crate::i18n::td_string!(l, claim.wallet.linked))}</span>
                                                    </div>
                                                    <button
                                                        class="btn btn-outline btn-sm claim-disconnect-btn"
                                                        on:click=move |_| set_change_wallet.set(true)
                                                        type="button"
                                                    >
                                                        {crate::locale::tr(|l| crate::i18n::td_string!(l, claim.wallet.use_different))}
                                                    </button>
                                                }.into_any()
                                            } else {
                                                let cw = connected_wallet.get();
                                                match cw {
                                                    // Connected state: wallet icon + name + truncated address + connected badge
                                                    Some((ref wallet_name, ref public_key)) => {
                                                        let wallet_icon = wallet_icon_name(wallet_name);
                                                        let pk_short = if public_key.len() > 12 {
                                                            format!("{}...{}", &public_key[..4], &public_key[public_key.len()-4..])
                                                        } else {
                                                            public_key.clone()
                                                        };
                                                        view! {
                                                            <div class="wallet-connected-bar">
                                                                <span class="wallet-icon-lg"><Icon icon=wallet_icon class="icon-lg" /></span>
                                                                <div class="wallet-info-left">
                                                                    <div class="wallet-label">{
                                                                        let wallet = wallet_name.clone();
                                                                        t!(i18n, claim.wallet.connected_via, wallet)
                                                                    }</div>
                                                                    <div class="wallet-address-bold">{pk_short}</div>
                                                                </div>
                                                                <span class="badge badge-success u-ml-auto"><Icon icon=IconName::Check class="icon-sm icon-success" />" "{crate::locale::tr(|l| crate::i18n::td_string!(l, claim.wallet.connected))}</span>
                                                            </div>
                                                            <button
                                                                class="btn btn-outline btn-sm claim-disconnect-btn"
                                                                on:click=move |_| { set_cw_for_connect.set(None); }
                                                                type="button"
                                                            >
                                                                {crate::locale::tr(|l| crate::i18n::td_string!(l, claim.wallet.disconnect))}
                                                            </button>
                                                        }.into_any()
                                                    }
                                                    // Not connected: show connect buttons + manual fallback
                                                    None => {
                                                        let mut wallets = detected_wallets.get();
                                                        // Always show Phantom as an option — if not installed,
                                                        // the connect will fail gracefully and show install prompt.
                                                        if !wallets.iter().any(|w| w.eq_ignore_ascii_case("Phantom")) {
                                                            wallets.push("Phantom".to_string());
                                                        }
                                                        let has_wallets = !wallets.is_empty();
                                                        view! {
                                                            // Wallet adapter connect buttons
                                                            {if has_wallets {
                                                                let wallets_for_click = wallets.clone();
                                                                view! {
                                                                    <div class="wallet-list">
                                                                        <p class="wallet-prompt">
                                                                            <Icon icon=IconName::Link class="icon-sm"/>
                                                                            " "{crate::locale::tr(|l| crate::i18n::td_string!(l, claim.wallet.connect_prompt))}
                                                                        </p>
                                                                        {wallets_for_click.into_iter().map(|w| {
                                                                            let w_clone = w.clone();
                                                                            let wallet_icon = wallet_icon_name(&w);
                                                                            view! {
                                                                                <button
                                                                                    class="btn btn-primary btn-block wallet-btn-inner"
                                                                                    on:click={
                                                                                        let w = w.clone();
                                                                                        let set_w = set_w_for_connect;
                                                                                        let set_cw = set_cw_for_connect;
                                                                                        move |_| {
                                                                                            let w = w.clone();
                                                                                            let set_w = set_w;
                                                                                            let set_cw = set_cw;
                                                                                            leptos::task::spawn_local(async move {
                                                                                                match connect_wallet_js(&w).await {
                                                                                                    crate::wallet_error::WalletResult::Success(pubkey) => {
                                                                                                        log::info!("[claim] wallet connected: {} ({})", w, pubkey);
                                                                                                        set_w.set(pubkey.clone());
                                                                                                        set_cw.set(Some((w, pubkey)));
                                                                                                    }
                                                                                                    crate::wallet_error::WalletResult::Error(e) => {
                                                                                                        if e.raw_message.contains("Wallet not found") {
                                                                                                            log::info!("[claim] {} not installed, opening download page", w);
                                                                                                            let url = match w.to_lowercase().as_str() {
                                                                                                                "phantom" => "https://phantom.app/download",
                                                                                                                "backpack" => "https://backpack.app/download",
                                                                                                                "solflare" => "https://solflare.com/download",
                                                                                                                _ => "https://phantom.app/download",
                                                                                                            };
                                                                                                            let _ = web_sys::window().and_then(|w| w.open_with_url_and_target(url, "_blank").ok());
                                                                                                        } else {
                                                                                                            log::warn!("[claim] wallet connect error for {}: code={:?} msg={}", w, e.code, e.raw_message);
                                                                                                        }
                                                                                                    }
                                                                                                    crate::wallet_error::WalletResult::UnknownFailure => {
                                                                                                        log::warn!("[claim] wallet connect failed for {}", w);
                                                                                                    }
                                                                                                }
                                                                                            });
                                                                                        }
                                                                                    }
                                                                                >
                                                                                    <span><Icon icon=wallet_icon class="icon-sm" /></span>
                                                                                    <span>{
                                                                                        let wallet = w_clone.clone();
                                                                                        t!(i18n, claim.wallet.connect_named, wallet)
                                                                                    }</span>
                                                                                </button>
                                                                            }
                                                                        }).collect::<Vec<_>>()}
                                                                    </div>
                                                                }.into_any()
                                                            } else {
                                                                view! { <div></div> }.into_any()
                                                            }}

                                                            // Divider — "or enter manually"
                                                            {if has_wallets {
                                                                view! {
                                                                    <div class="claim-wallet-divider">
                                                                        <span>{crate::locale::tr(|l| crate::i18n::td_string!(l, claim.wallet.or_manual))}</span>
                                                                    </div>
                                                                }.into_any()
                                                            } else {
                                                                view! { <div></div> }.into_any()
                                                            }}

                                                            // Manual text input (always visible as fallback)
                                                            <div class="claim-wallet-row">
                                                                <input
                                                                    class="claim-wallet-input"
                                                                    type="text"
                                                                    placeholder=crate::locale::tr(|l| crate::i18n::td_string!(l, claim.wallet.placeholder))
                                                                    prop:value=move || wallet_input.get()
                                                                    on:input=move |ev| {
                                                                        let val = event_target_value(&ev);
                                                                        set_wallet_input.set(val);
                                                                    }
                                                                />
                                                                <button
                                                                    class="claim-paste-btn"
                                                                    on:click=handle_paste
                                                                    type="button"
                                                                >
                                                                    {crate::locale::tr(|l| crate::i18n::td_string!(l, claim.wallet.paste))}
                                                                </button>
                                                            </div>
                                                            <p class="claim-wallet-hint">
                                                                {
                                                                    match &locked_wallet {
                                                                        Some(w) if !w.is_empty() => crate::locale::tr(|l| crate::i18n::td_string!(l, claim.wallet.hint_locked)).into_any(),
                                                                        _ if has_suggested_wallet => crate::locale::tr(|l| crate::i18n::td_string!(l, claim.wallet.hint_elsewhere)).into_any(),
                                                                        _ => crate::locale::tr(|l| crate::i18n::td_string!(l, claim.wallet.hint_paste)).into_any(),
                                                                    }
                                                                }
                                                            </p>
                                                        }.into_any()
                                                    }
                                                }
                                            }
                                        }}
                                    </div>

                                    // Claim button
                                    <button
                                        class="claim-btn-mint"
                                        on:click=handle_claim
                                        disabled=move || {
                                            // The one-tap linked path needs no input — the wallet is
                                            // resolved server-side. Only gate the explicit-wallet path.
                                            let use_linked = has_linked_wallet.get()
                                                && !change_wallet.get()
                                                && connected_wallet.get().is_none();
                                            if use_linked {
                                                return false;
                                            }
                                            let w = wallet_input.get();
                                            let w_trimmed = w.trim();
                                            w_trimmed.is_empty() || !(32..=44).contains(&w_trimmed.len())
                                        }
                                    >
                                        {crate::locale::tr(|l| crate::i18n::td_string!(l, claim.mint_cta))}
                                    </button>
                                </div>
                            }
                                .into_any()
                        }

                        // ---- Adventure gate — must complete before claiming ----
                        ClaimState::Adventure(data, adv_status) => {
                            let token_val = match params.get() {
                                Ok(p) => p.token.unwrap_or_default(),
                                Err(_) => String::new(),
                            };
                            let adventure_url = if data.event_id.is_empty() {
                                format!("/adventure?token={token_val}")
                            } else {
                                format!("/adventure?token={token_val}&event_id={}", data.event_id)
                            };
                            let status_msg = match adv_status {
                                AdventureStatusType::NotStarted => crate::locale::tr(|l| crate::i18n::td_string!(l, claim.adventure.not_started)).into_any(),
                                AdventureStatusType::InProgress => crate::locale::tr(|l| crate::i18n::td_string!(l, claim.adventure.in_progress)).into_any(),
                                _ => crate::locale::tr(|l| crate::i18n::td_string!(l, claim.adventure.complete_it)).into_any(),
                            };
                            let adventure_name = escape_html(&data.name);
                            view! {
                                <div class="claim-adventure-gate">
                                    <ParticipantAvatar name=data.name.clone() />
                                    <h2><Icon icon=IconName::Crab class="icon-md" />" "{crate::locale::tr(|l| crate::i18n::td_string!(l, claim.adventure.title))}</h2>
                                    <p class="claim-adventure-status">{status_msg}</p>
                                    <div class="claim-adventure-info">
                                        <p>
                                            {t!(
                                                i18n,
                                                claim.adventure.info,
                                                <strong> = |children: ChildrenFn| view! { <strong>{children()}</strong> },
                                                name = adventure_name
                                            )}
                                        </p>
                                        <p class="claim-adventure-hint">
                                            {crate::locale::tr(|l| crate::i18n::td_string!(l, claim.adventure.hint))}
                                        </p>
                                    </div>
                                    <a
                                        class="btn btn-primary claim-adventure-btn"
                                        href={adventure_url}
                                    >
                                        {crate::locale::tr(|l| crate::i18n::td_string!(l, claim.adventure.start))}
                                    </a>
                                </div>
                            }
                                .into_any()
                        }

                        // ---- Minting in progress ----
                        ClaimState::Minting(data) => {
                            view! {
                                <div class="claim-minting">
                                    // Pulsing minting indicator
                                    <div class="claim-minting-spinner">
                                        <div class="shimmer claim-minting-shimmer"></div>
                                        <span class="spinner spinner-lg"></span>
                                    </div>
                                    <h3 class="claim-minting-title">{crate::locale::tr(|l| crate::i18n::td_string!(l, claim.minting.title))}</h3>
                                    <p class="claim-minting-detail">
                                        {
                                            let name = escape_html(&data.name);
                                            t!(i18n, claim.minting.detail, name)
                                        }
                                    </p>
                                    <p class="claim-minting-hint">
                                        {crate::locale::tr(|l| crate::i18n::td_string!(l, claim.minting.hint))}
                                    </p>
                                </div>
                            }
                                .into_any()
                        }

                        // ---- Success! ----
                        ClaimState::Success(data) => {
                            let api_id = deposit_api_id.get();
                            let event_id = deposit_event_id.get();
                            let ticket_href = match event_id.is_empty() {
                                true => format!("/ticket/{api_id}"),
                                false => format!("/ticket/{api_id}?event_id={event_id}"),
                            };
                            let claim_token = params
                                .get()
                                .ok()
                                .and_then(|p| p.token)
                                .unwrap_or_default();
                            view! {
                                <super::success::ClaimSuccess
                                    data=data
                                    event_name=evt_name.get()
                                    claim_token=claim_token
                                    ticket_href=ticket_href
                                    deposit_enabled=deposit_enabled.get()
                                />
                            }
                            .into_any()
                        }

                        // ---- Already claimed ---- redirect to ticket page
                        ClaimState::AlreadyClaimed(_data) => {
                            // Build ticket page URL from deposit signals
                            let api_id = deposit_api_id.get();
                            let event_id = deposit_event_id.get();
                            let ticket_href = if event_id.is_empty() {
                                format!("/ticket/{api_id}")
                            } else {
                                format!("/ticket/{api_id}?event_id={event_id}")
                            };

                            // Redirect immediately via JS
                            let href_clone = ticket_href.clone();
                            leptos::task::spawn_local(async move {
                                if let Some(window) = web_sys::window() {
                                    let _ = window.location().set_href(&href_clone);
                                }
                            });

                            view! {
                                <div class="claim-state-full claim-redirect-center">
                                                                    <span class="spinner spinner-lg claim-redirect-spinner"></span>
                                                                    <h3 class="claim-redirect-title">{crate::locale::tr(|l| crate::i18n::td_string!(l, claim.already.title))}</h3>
                                                                    <p class="claim-redirect-desc">
                                                                        {crate::locale::tr(|l| crate::i18n::td_string!(l, claim.already.redirecting))}
                                                                    </p>
                                                                    <a
                                                                        href=ticket_href
                                                                        class="btn btn-outline claim-redirect-btn"
                                    >
                                        {crate::locale::tr(|l| crate::i18n::td_string!(l, claim.already.go_to_ticket))}
                                    </a>
                                </div>
                            }
                                .into_any()
                        }

                        // ---- Mint error ----
                        ClaimState::MintError(data, failure) => {
                            let (pending, error) = match failure {
                                super::mint_retry::MintFailure::Failed(message) => (false, escape_html(&message)),
                                super::mint_retry::MintFailure::StillPending => (true, String::new()),
                            };
                            let pending_detail = crate::locale::tr(|l| crate::i18n::td_string!(l, claim.mint_error.pending_detail));
                            let (tone, title) = match pending {
                                true => (crate::components::StatusTone::Pending, crate::locale::tr(|l| crate::i18n::td_string!(l, claim.mint_error.pending_title))),
                                false => (crate::components::StatusTone::Failed, crate::locale::tr(|l| crate::i18n::td_string!(l, claim.mint_error.title))),
                            };
                            view! {
                                <div class="claim-error">
                                    {move || view! {
                                        <crate::components::StatusBadge
                                            tone=tone
                                            label=match pending {
                                                true => t_string!(i18n, claim.mint_error.pending_title),
                                                false => t_string!(i18n, claim.mint_error.badge),
                                            }
                                        />
                                    }}
                                    <h2>{title}</h2>
                                    <div class="result-details">
                                        <p>{move || match pending {
                                            true => pending_detail.get().to_string(),
                                            false => error.clone(),
                                        }}</p>
                                        <p>
                                            {crate::locale::tr(|l| crate::i18n::td_string!(l, claim.mint_error.retry_note))}
                                        </p>
                                    </div>
                                    <button
                                        class="btn btn-primary claim-retry-btn"
                                        on:click=move |_| {
                                            set_state.set(ClaimState::Ready(data.clone()));
                                        }
                                    >
                                        {crate::locale::tr(|l| crate::i18n::td_string!(l, claim.mint_error.retry))}
                                    </button>
                                </div>
                            }
                                .into_any()
                        }
                    }
                }}

                // Fun: hearts reaction widget (only on loaded/engaged states)
                {move || {
                    match state.get() {
                        ClaimState::NftComingSoon(_) |
                        ClaimState::Ready(_) |
                        ClaimState::Success(_) |
                        ClaimState::AlreadyClaimed(_) => {
                            view! { <HeartsWidget /> }.into_any()
                        }
                        _ => view! { <div></div> }.into_any()
                    }
                }}

                // Footer
                <div class="claim-footer">
                    <div class="brand-line">
                        <span class="accent">"BeThere"</span>
                        " x Solana Thailand"
                    </div>
                </div>
            </div>
        </div>
    }
}
