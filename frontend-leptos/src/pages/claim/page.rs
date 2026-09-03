//! Claim page component — public route at `/claim/:token`.

use leptos::prelude::*;
use leptos_meta::Title;
use leptos_router::hooks::use_params;

use crate::api::{
    self, AdventureStatusType, QuizStatus,
};
use crate::icons::{Icon, IconName, wallet_icon_name};
use crate::utils::{escape_html, orb_nft_url};

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
    let (quiz_answers, set_quiz_answers): (ReadSignal<QuizAnswers>, WriteSignal<QuizAnswers>) = signal(QuizAnswers::new());

    // Dynamic event config (fetched from backend, replaces hardcoded values)
    let (evt_name, set_evt_name) = signal(String::new());
    let (evt_tagline, set_evt_tagline) = signal(String::new());
    let (evt_link, set_evt_link) = signal(String::new());
    let (evt_start, set_evt_start) = signal(0i64);
    let (evt_end, set_evt_end) = signal(0i64);

    // Share feedback
    let (share_copied, set_share_copied) = signal(false);

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
                set_state.set(ClaimState::NotFound(
                    "Invalid claim link — missing token.".to_string(),
                ));
                return;
            }
        };

        if token.is_empty() {
            set_state.set(ClaimState::NotFound(
                "Invalid claim link — missing token.".to_string(),
            ));
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
                            match api::get_adventure_status(&token_for_adventure, Some(&claim_data_for_adventure.event_id)).await {
                                Ok(status_data) => {
                                    match status_data.status {
                                        AdventureStatusType::NotRequired => {
                                            set_state.set(ClaimState::Ready(claim_data_for_adventure));
                                        }
                                        AdventureStatusType::Passed => {
                                            set_state.set(ClaimState::Ready(claim_data_for_adventure));
                                        }
                                        AdventureStatusType::NotStarted | AdventureStatusType::InProgress => {
                                            log::info!("[claim] adventure required but not passed, showing adventure gate");
                                            set_state.set(ClaimState::Adventure(
                                                claim_data_for_adventure,
                                                status_data.status,
                                            ));
                                        }
                                    }
                                }
                                Err(e) => {
                                    log::warn!("[claim] failed to check adventure status: {e}, proceeding to Ready");
                                    // Fallback to Ready so attendee isn't stuck
                                    set_state.set(ClaimState::Ready(claim_data_for_adventure));
                                }
                            }
                        });
                    }
                }
                Err(e) => {
                    log::warn!("[claim] lookup failed for token {token}: {e}");
                    set_state.set(ClaimState::NotFound(format!(
                        "Claim token not found or lookup failed: {e}"
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
            let arg = if use_linked { None } else { Some(wallet.as_str()) };
            let result = api::post_claim(&token, arg, use_linked).await;
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
                Err(e) => {
                    log::error!("[claim] mint failed: {e}");
                    set_state.set(ClaimState::MintError(current_data_clone, format!("{e}")));
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

    view! {
        <div class="center-page">
            <Title text="Claim Your NFT — BeThere" />
            <div class="container claim-container">
                // Brand header
                <div class="brand-logo">"BeThere"</div>
                <div class="brand-logo-sub">"Proof of Attendance"</div>

                // Title
                <h1 class="claim-title">"Claim Your NFT"</h1>

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
                        ClaimState::NotFound(msg) => {
                            view! {
                                <div class="claim-error">
                                    <h2>"Claim Not Found"</h2>
                                    <div class="result-details">
                                        <p>{escape_html(&msg)}</p>
                                    </div>
                                    <a href="/" class="btn btn-outline claim-retry-btn">
                                        "Go to Home"
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
                                    "Waiting for Quiz Setup",
                                    "Your organizer is preparing a quiz for this event. You’ll be able to claim your NFT badge once it’s ready.",
                                    "Please check back soon or remind your organizer to set up the quiz!"
                                )
                            } else {
                                (
                                    "NFT Badge Coming Soon",
                                    "Your proof-of-attendance NFT badge is being prepared.",
                                    "You will receive a compressed NFT on Solana — a permanent, on-chain proof that you attended this event."
                                )
                            };
                            view! {
                                <div class="claim-state-full">
                                    // Attendee welcome
                                    <div class="claim-welcome-card">
                                        <ParticipantAvatar name=data.name.clone() />
                                        <h3>"Welcome, "{escape_html(&data.name)}"!"</h3>
                                        <p class="checked-in-label">{checked_in_display}</p>
                                    </div>

                                    // NFT badge preview
                                    <NftBadgePreview />

                                    // Status card — quiz pending or NFT coming soon
                                    <div class="claim-nft-soon-card">
                                        <h3>{card_title}</h3>
                                        <p>{card_msg}</p>
                                        <div class="nft-description">
                                            {card_detail}
                                        </div>
                                    </div>

                                    // Compact wallet hint
                                    <p class="claim-bookmark-hint">
                                        "Get a "
                                        <a href="https://phantom.app/" target="_blank" rel="noopener noreferrer">"Solana wallet"</a>
                                        " ready — bookmark this page to claim your NFT later."
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
                                        <h3>"Welcome, "{escape_html(&data.name)}"!"</h3>
                                        <p class="checked-in-label">{checked_in_display}</p>
                                    </div>

                                    // NFT badge preview
                                    <NftBadgePreview />

                                    // Wallet input — wallet adapter + manual fallback
                                    <div class="card">
                                        <label class="claim-wallet-label">
                                            "Solana Wallet Address"
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
                                                            <div class="wallet-label">"Your linked wallet"</div>
                                                            <div class="wallet-address-bold">{truncated}</div>
                                                        </div>
                                                        <span class="badge badge-success u-ml-auto"><Icon icon=IconName::Check class="icon-sm icon-success" />" Linked"</span>
                                                    </div>
                                                    <button
                                                        class="btn btn-outline btn-sm claim-disconnect-btn"
                                                        on:click=move |_| set_change_wallet.set(true)
                                                        type="button"
                                                    >
                                                        "Use a different wallet"
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
                                                                    <div class="wallet-label">"Connected via " {wallet_name.clone()}</div>
                                                                    <div class="wallet-address-bold">{pk_short}</div>
                                                                </div>
                                                                <span class="badge badge-success u-ml-auto"><Icon icon=IconName::Check class="icon-sm icon-success" />" Connected"</span>
                                                            </div>
                                                            <button
                                                                class="btn btn-outline btn-sm claim-disconnect-btn"
                                                                on:click=move |_| { set_cw_for_connect.set(None); }
                                                                type="button"
                                                            >
                                                                "Disconnect"
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
                                                                            " Connect your Solana wallet:"
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
                                                                                    <span>{format!("Connect {}", w_clone)}</span>
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
                                                                        <span>"or enter manually"</span>
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
                                                                    placeholder="Enter your Solana wallet address"
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
                                                                    "Paste"
                                                                </button>
                                                            </div>
                                                            <p class="claim-wallet-hint">
                                                                {
                                                                    match &locked_wallet {
                                                                        Some(w) if !w.is_empty() => "Use the pre-filled wallet address to claim.",
                                                                        _ if has_suggested_wallet => "Connect or enter the wallet you'd like the badge sent to instead of your linked one.",
                                                                        _ => "Tap Paste or type your Phantom, Solflare, or Backpack address.",
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
                                        "Claim NFT Badge"
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
                                AdventureStatusType::NotStarted => "You haven't started the Rust Adventure yet. Complete it to unlock your NFT!",
                                AdventureStatusType::InProgress => "You're making progress! Keep going to complete the adventure.",
                                _ => "Complete the Rust Adventure to unlock your NFT!",
                            };
                            view! {
                                <div class="claim-adventure-gate">
                                    <ParticipantAvatar name=data.name.clone() />
                                    <h2><Icon icon=IconName::Crab class="icon-md" />" Rust Adventure Required"</h2>
                                    <p class="claim-adventure-status">{status_msg}</p>
                                    <div class="claim-adventure-info">
                                        <p>
                                            <strong>{escape_html(&data.name)}</strong>", complete the Rust Adventures game to earn your NFT badge."
                                        </p>
                                        <p class="claim-adventure-hint">
                                            "Learn Rust basics by solving coding puzzles in a fun tile-based game!"
                                        </p>
                                    </div>
                                    <a
                                        class="btn btn-primary claim-adventure-btn"
                                        href={adventure_url}
                                    >
                                        "🎮 Start Adventure"
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
                                    <h3 class="claim-minting-title">"Minting your NFT..."</h3>
                                    <p class="claim-minting-detail">
                                        "Minting for "{escape_html(&data.name)}
                                    </p>
                                    <p class="claim-minting-hint">
                                        "This usually takes 3-5 seconds."
                                    </p>
                                </div>
                            }
                                .into_any()
                        }

                        // ---- Success! ----
                        ClaimState::Success(data) => {
                            let orb_url = orb_nft_url(&data.asset_id, &data.cluster);
                            let asset_id_display = {
                                let id = &data.asset_id;
                                if id.len() > 12 {
                                    format!("{}...{}", &id[..6], &id[id.len()-4..])
                                } else {
                                    id.clone()
                                }
                            };
                            let asset_id_full = data.asset_id.clone();

                            // Build share text & URL
                            let tweet_text = {
                                let event = evt_name.get();
                                if event.is_empty() {
                                    "I just claimed my attendance NFT on BeThere! 🎫✨\n\nProof I showed up. On-chain.\n\n#BeThere #Solana".to_string()
                                } else {
                                    format!("I just earned my POAP at {event}! 🎫✨\n\nShowed up, proved I was there, got my NFT badge.\n\n#BeThere #Solana")
                                }
                            };
                            let share_to_x_url = format!(
                                "https://twitter.com/intent/tweet?text={}",
                                js_sys::encode_uri_component(&tweet_text)
                            );
                            let claim_page_url = format!(
                                "https://bethere.solana-thailand.workers.dev/claim/{}",
                                match params.get() {
                                    Ok(p) => p.token.unwrap_or_default(),
                                    Err(_) => String::new(),
                                }
                            );

                            let solscan_url = if data.signature.is_empty() {
                                format!("https://solscan.io/account/{}?cluster={}", data.asset_id, data.cluster)
                            } else {
                                format!("https://solscan.io/tx/{}?cluster={}", data.signature, data.cluster)
                            };

                            view! {
                                <div class="claim-success">
                                    // 1. Celebration
                                    <div class="claim-success-rings">
                                        <div class="claim-success-ring claim-success-ring-3"></div>
                                        <div class="claim-success-ring claim-success-ring-2"></div>
                                        <div class="claim-success-ring claim-success-ring-1"></div>
                                        <div class="success-check">
                                            <svg viewBox="0 0 24 24">
                                                <polyline points="20 6 9 17 4 12"></polyline>
                                            </svg>
                                        </div>
                                    </div>
                                    <h2>"NFT Claimed!"</h2>

                                    // 2. Asset ID + View NFT
                                    <div class="claim-asset-card">
                                        <div class="claim-asset-header">
                                            <span class="claim-asset-label">"Asset ID"</span>
                                            <span class="claim-asset-status">
                                                <span class="claim-asset-status-dot"></span>
                                                "On-Chain"
                                            </span>
                                        </div>
                                        <div class="claim-asset-value-row">
                                            <span class="claim-asset-code">{asset_id_display}</span>
                                            <button
                                                class="claim-copy-btn"
                                                type="button"
                                                title="Copy Asset ID"
                                                on:click=move |_| {
                                                    let _ = copy_to_clipboard_js(&asset_id_full);
                                                }
                                            >
                                                <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                                                    <rect x="9" y="9" width="13" height="13" rx="2" ry="2"></rect>
                                                    <path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1"></path>
                                                </svg>
                                            </button>
                                        </div>
                                    </div>

                                    <div class="success-actions" style="display: flex; flex-direction: column; gap: 10px;">
                                        <a
                                            href=orb_url
                                            target="_blank"
                                            rel="noopener noreferrer"
                                            class="btn btn-primary btn-block"
                                        >
                                            "View NFT on Orb ↗"
                                        </a>
                                        <a
                                            href=solscan_url
                                            target="_blank"
                                            rel="noopener noreferrer"
                                            class="btn btn-outline btn-block"
                                            style="border-color: rgba(20, 241, 149, 0.4); color: #14F195; background: rgba(20, 241, 149, 0.06);"
                                        >
                                            "🔍 View Transaction on Solscan ↗"
                                        </a>
                                    </div>

                                    // 3. Share (compact row)
                                    <div class="claim-share-section">
                                    <div class="claim-share-buttons">
                                        <a
                                            href=share_to_x_url
                                            target="_blank"
                                            rel="noopener noreferrer"
                                            class="claim-share-x-btn"
                                        >
                                            <svg viewBox="0 0 24 24" fill="currentColor">
                                                <path d="M18.244 2.25h3.308l-7.227 8.26 8.502 11.24H16.17l-5.214-6.817L4.99 21.75H1.68l7.73-8.835L1.254 2.25H8.08l4.713 6.231zm-1.161 17.52h1.833L7.084 4.126H5.117z"/>
                                            </svg>
                                            "Post to X"
                                        </a>
                                        <button
                                            class="claim-share-copy-btn"
                                            type="button"
                                            title="Copy claim link"
                                            on:click=move |_| {
                                                let _ = copy_to_clipboard_js(&claim_page_url);
                                                set_share_copied.set(true);
                                                leptos::task::spawn_local(async move {
                                                    gloo_timers::future::TimeoutFuture::new(2000).await;
                                                    set_share_copied.set(false);
                                                });
                                            }
                                        >
                                            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                                                <path d="M10 13a5 5 0 0 0 7.54.54l3-3a5 5 0 0 0-7.07-7.07l-1.72 1.71"></path>
                                                <path d="M14 11a5 5 0 0 0-7.54-.54l-3 3a5 5 0 0 0 7.07 7.07l1.71-1.71"></path>
                                            </svg>
                                        </button>
                                    </div>
                                    <div class={move || {
                                        if share_copied.get() {
                                            "claim-share-copied visible".to_string()
                                        } else {
                                            "claim-share-copied".to_string()
                                        }
                                    }}>
                                        "Link copied!"
                                    </div>
                                    </div>

                                    // 4. Ticket link — the ticket page is the hub for
                                    // deposit & refund status (per-method actions live there;
                                    // the deposit page is only for PAYING a deposit, so linking
                                    // there post-claim dead-ended non-depositors on a pay form).
                                    {
                                        let api_id = deposit_api_id.get();
                                        let event_id = deposit_event_id.get();
                                        let ticket_href = if event_id.is_empty() {
                                            format!("/ticket/{api_id}")
                                        } else {
                                            format!("/ticket/{api_id}?event_id={event_id}")
                                        };
                                        let label = if deposit_enabled.get() {
                                            "View ticket & deposit / refund →"
                                        } else {
                                            "← Back to Ticket"
                                        };
                                        view! {
                                            <div class="success-actions claim-success-actions-spaced">
                                                <a
                                                    href=ticket_href
                                                    class="btn btn-outline btn-block"
                                                >
                                                    {label}
                                                </a>
                                            </div>
                                        }.into_any()
                                    }
                                </div>
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
                                                                    <h3 class="claim-redirect-title">"Already Claimed"</h3>
                                                                    <p class="claim-redirect-desc">
                                                                        "Redirecting to your ticket..."
                                                                    </p>
                                                                    <a
                                                                        href=ticket_href
                                                                        class="btn btn-outline claim-redirect-btn"
                                    >
                                        "Go to Ticket"
                                    </a>
                                </div>
                            }
                                .into_any()
                        }

                        // ---- Mint error ----
                        ClaimState::MintError(data, error) => {
                            view! {
                                <div class="claim-error">
                                    <h2>"Minting Failed"</h2>
                                    <div class="result-details">
                                        <p>{escape_html(&error)}</p>
                                    </div>
                                    <button
                                        class="btn btn-primary claim-retry-btn"
                                        on:click=move |_| {
                                            set_state.set(ClaimState::Ready(data.clone()));
                                        }
                                    >
                                        "Try Again"
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
