//! Interactive role-based demo page — lets visitors experience BeThere from every perspective.
//!
//! Users select a role (Organizer, Staff, or Attendee) and walk through a step-by-step
//! simulated journey showing what that role does. Each role has a representative persona
//! and a unique flow that highlights BeThere's value from that viewpoint.

use leptos::prelude::*;
use leptos_router::components::A;

// ── Roles ──────────────────────────────────────────────────────────────────────

#[derive(Clone, Copy, PartialEq, Eq)]
enum DemoRole {
    Picker,
    Organizer,
    Staff,
    Attendee,
}

impl DemoRole {
    fn emoji(self) -> &'static str {
        match self {
            DemoRole::Organizer => "🎯",
            DemoRole::Staff => "📱",
            DemoRole::Attendee => "🎫",
            DemoRole::Picker => "🤔",
        }
    }

    fn title(self) -> &'static str {
        match self {
            DemoRole::Organizer => "Organizer",
            DemoRole::Staff => "Event Staff",
            DemoRole::Attendee => "Attendee",
            DemoRole::Picker => "",
        }
    }

    fn persona_name(self) -> &'static str {
        match self {
            DemoRole::Organizer => "Priya S.",
            DemoRole::Staff => "Ken M.",
            DemoRole::Attendee => "Alex C.",
            DemoRole::Picker => "",
        }
    }

    fn persona_initials(self) -> &'static str {
        match self {
            DemoRole::Organizer => "PS",
            DemoRole::Staff => "KM",
            DemoRole::Attendee => "AC",
            DemoRole::Picker => "",
        }
    }

    fn persona_title(self) -> &'static str {
        match self {
            DemoRole::Organizer => "Community Lead, Solana Bangkok",
            DemoRole::Staff => "Volunteer, Door Check-in",
            DemoRole::Attendee => "Developer & Web3 Enthusiast",
            DemoRole::Picker => "",
        }
    }

    fn persona_goal(self) -> &'static str {
        match self {
            DemoRole::Organizer => {
                "You're running a 200-person meetup and tired of 40% no-shows. You need accountability — and a way to prove who actually showed up."
            }
            DemoRole::Staff => {
                "You're volunteering at the registration desk. Your job is fast check-in — scan each attendee's QR code and confirm them in seconds."
            }
            DemoRole::Attendee => {
                "You registered for a Solana meetup with a small deposit. Now you're at the venue, ready to check in and get your money back."
            }
            DemoRole::Picker => "",
        }
    }

    fn accent_color(self) -> &'static str {
        match self {
            DemoRole::Organizer => "#6366f1", // indigo
            DemoRole::Staff => "#f59e0b",     // amber
            DemoRole::Attendee => "#22c55e",  // green
            DemoRole::Picker => "#6366f1",
        }
    }

    fn accent_bg(self) -> &'static str {
        match self {
            DemoRole::Organizer => "rgba(99,102,241,0.15)",
            DemoRole::Staff => "rgba(245,158,11,0.15)",
            DemoRole::Attendee => "rgba(34,197,94,0.15)",
            DemoRole::Picker => "rgba(99,102,241,0.15)",
        }
    }

    fn accent_border(self) -> &'static str {
        match self {
            DemoRole::Organizer => "rgba(99,102,241,0.3)",
            DemoRole::Staff => "rgba(245,158,11,0.3)",
            DemoRole::Attendee => "rgba(34,197,94,0.3)",
            DemoRole::Picker => "rgba(99,102,241,0.3)",
        }
    }
}

// ── Journey Steps ──────────────────────────────────────────────────────────────

#[derive(Clone, Copy, PartialEq, Eq)]
enum OrganizerStep {
    Setup,
    Dashboard,
    LiveCheckin,
    Payout,
    Done,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum StaffStep {
    Scan,
    Confirmed,
    Manual,
    Done,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum AttendeeStep {
    Arrive,
    Scanned,
    Quiz,
    Claim,
    Done,
}

// ── Demo Data ──────────────────────────────────────────────────────────────────

struct DemoEvent;

impl DemoEvent {
    fn name() -> &'static str { "Solana Bangkok Meetup 2025" }
    fn date() -> &'static str { "Jul 15, 2025" }
    fn location() -> &'static str { "Bangkok, Thailand" }
    fn capacity() -> u32 { 200 }
    fn deposit() -> &'static str { "0.01 SOL + $13.00 USDC" }
}

struct DemoQuiz;

impl DemoQuiz {
    fn question() -> &'static str {
        "What does BeThere use to prove attendance on-chain?"
    }
    fn options() -> [&'static str; 4] {
        ["PDF certificate", "Compressed NFT badge", "Email receipt", "Paper ticket"]
    }
}

// ── Demo Component ─────────────────────────────────────────────────────────────

/// Demo page — role-based interactive walkthrough.
#[component]
pub fn Demo() -> impl IntoView {
    let (role, set_role) = signal(DemoRole::Picker);
    let (org_step, set_org_step) = signal(OrganizerStep::Setup);
    let (staff_step, set_staff_step) = signal(StaffStep::Scan);
    let (attendee_step, set_attendee_step) = signal(AttendeeStep::Arrive);
    let (simulating, set_simulating) = signal(false);
    let (quiz_answer, set_quiz_answer) = signal(None::<usize>);
    let (quiz_submitted, set_quiz_submitted) = signal(false);

    let pick_role = move |r: DemoRole| {
        set_role.set(r);
        // Reset all steps
        set_org_step.set(OrganizerStep::Setup);
        set_staff_step.set(StaffStep::Scan);
        set_attendee_step.set(AttendeeStep::Arrive);
        set_quiz_answer.set(None);
        set_quiz_submitted.set(false);
        set_simulating.set(false);
    };

    view! {
        <div style="min-height:100vh;width:100%;background:var(--bg-primary);">

            // ── Nav ──
            <nav style="position:sticky;top:0;z-index:100;background:rgba(15,15,15,0.85);backdrop-filter:blur(12px);border-bottom:1px solid var(--border);">
                <div style="max-width:960px;margin:0 auto;padding:0.85rem 1.5rem;display:flex;align-items:center;justify-content:space-between;">
                    <A href="/" attr:style="text-decoration:none;">
                        <span style="font-size:1.25rem;font-weight:800;letter-spacing:0.06em;background:linear-gradient(135deg,#818cf8 0%,#6366f1 40%,#a78bfa 100%);-webkit-background-clip:text;-webkit-text-fill-color:transparent;background-clip:text;">
                            "BeThere"
                        </span>
                    </A>
                    <div style="display:flex;align-items:center;gap:0.75rem;">
                        <Show when=move || role.get() != DemoRole::Picker>
                            <button
                                class="btn btn-outline btn-sm"
                                on:click=move |_| set_role.set(DemoRole::Picker)
                            >
                                {"\u{2190} Switch Role"}
                            </button>
                        </Show>
                        <span style="font-size:0.75rem;font-weight:600;color:var(--warning);background:rgba(245,158,11,0.1);border:1px solid rgba(245,158,11,0.3);border-radius:9999px;padding:0.25rem 0.75rem;">
                            "Demo"
                        </span>
                    </div>
                </div>
            </nav>

            // ── Content ──
            <div style="max-width:560px;margin:0 auto;padding:2rem 1.5rem;">

                // ============== ROLE PICKER ==============
                <Show when=move || role.get() == DemoRole::Picker>
                    {role_picker(pick_role)}
                </Show>

                // ============== ORGANIZER JOURNEY ==============
                <Show when=move || role.get() == DemoRole::Organizer>
                    {organizer_journey(
                        role,
                        org_step,
                        set_org_step,
                        simulating,
                        set_simulating,
                    )}
                </Show>

                // ============== STAFF JOURNEY ==============
                <Show when=move || role.get() == DemoRole::Staff>
                    {staff_journey(
                        role,
                        staff_step,
                        set_staff_step,
                        simulating,
                        set_simulating,
                    )}
                </Show>

                // ============== ATTENDEE JOURNEY ==============
                <Show when=move || role.get() == DemoRole::Attendee>
                    {attendee_journey(
                        role,
                        attendee_step,
                        set_attendee_step,
                        simulating,
                        set_simulating,
                        quiz_answer,
                        set_quiz_answer,
                        quiz_submitted,
                        set_quiz_submitted,
                    )}
                </Show>

            </div>
        </div>
    }
}

// ── Role Picker View ───────────────────────────────────────────────────────────

fn role_picker(on_pick: impl Fn(DemoRole) + Clone + 'static) -> impl IntoView {
    view! {
        // Header
        <div style="text-align:center;margin-bottom:2.5rem;">
            <h1 style="font-size:1.5rem;font-weight:800;color:#fff;margin-bottom:0.5rem;">
                "Experience BeThere"
            </h1>
            <p style="font-size:0.95rem;color:var(--text-secondary);line-height:1.6;">
                "Choose your role to see what it's like. Each person interacts with BeThere differently."
            </p>
        </div>

        // Role cards
        <div style="display:flex;flex-direction:column;gap:1rem;">

            // ── Organizer Card ──
            <button
                style="width:100%;text-align:left;background:var(--bg-secondary);border:1px solid var(--border);border-radius:var(--radius);padding:1.5rem;cursor:pointer;transition:border-color 0.2s;"
                on:click={
                    let on_pick = on_pick.clone();
                    move |_| on_pick(DemoRole::Organizer)
                }
            >
                <div style="display:flex;align-items:flex-start;gap:1rem;">
                    <div style="width:52px;height:52px;border-radius:12px;background:rgba(99,102,241,0.15);border:1px solid rgba(99,102,241,0.3);display:flex;align-items:center;justify-content:center;font-size:1.5rem;flex-shrink:0;">
                        {"\u{1f3af}"}
                    </div>
                    <div style="flex:1;min-width:0;">
                        <div style="display:flex;align-items:center;gap:0.5rem;margin-bottom:0.25rem;">
                            <span style="font-size:1.1rem;font-weight:700;color:#fff;">"Organizer"</span>
                            <span style="font-size:0.7rem;padding:0.15rem 0.5rem;background:rgba(99,102,241,0.15);color:#818cf8;border:1px solid rgba(99,102,241,0.3);border-radius:9999px;">"Creates events"</span>
                        </div>
                        <p style="font-size:0.85rem;color:var(--text-secondary);line-height:1.5;margin-bottom:0.5rem;">
                            "You're Priya, a community lead hosting a 200-person meetup. See how to set deposits, track attendance in real-time, and collect no-show funds."
                        </p>
                        <div style="display:flex;flex-wrap:wrap;gap:0.4rem;">
                            <span style="font-size:0.7rem;color:var(--text-muted);background:var(--bg-tertiary);padding:0.15rem 0.5rem;border-radius:9999px;">"Set deposit"</span>
                            <span style="font-size:0.7rem;color:var(--text-muted);background:var(--bg-tertiary);padding:0.15rem 0.5rem;border-radius:9999px;">"Live dashboard"</span>
                            <span style="font-size:0.7rem;color:var(--text-muted);background:var(--bg-tertiary);padding:0.15rem 0.5rem;border-radius:9999px;">"Collect no-show funds"</span>
                        </div>
                    </div>
                </div>
            </button>

            // ── Staff Card ──
            <button
                style="width:100%;text-align:left;background:var(--bg-secondary);border:1px solid var(--border);border-radius:var(--radius);padding:1.5rem;cursor:pointer;transition:border-color 0.2s;"
                on:click={
                    let on_pick = on_pick.clone();
                    move |_| on_pick(DemoRole::Staff)
                }
            >
                <div style="display:flex;align-items:flex-start;gap:1rem;">
                    <div style="width:52px;height:52px;border-radius:12px;background:rgba(245,158,11,0.15);border:1px solid rgba(245,158,11,0.3);display:flex;align-items:center;justify-content:center;font-size:1.5rem;flex-shrink:0;">
                        {"\u{1f4f1}"}
                    </div>
                    <div style="flex:1;min-width:0;">
                        <div style="display:flex;align-items:center;gap:0.5rem;margin-bottom:0.25rem;">
                            <span style="font-size:1.1rem;font-weight:700;color:#fff;">"Event Staff"</span>
                            <span style="font-size:0.7rem;padding:0.15rem 0.5rem;background:rgba(245,158,11,0.15);color:#f59e0b;border:1px solid rgba(245,158,11,0.3);border-radius:9999px;">"Checks people in"</span>
                        </div>
                        <p style="font-size:0.85rem;color:var(--text-secondary);line-height:1.5;margin-bottom:0.5rem;">
                            {"You're Ken, a volunteer at the door. See how fast QR scanning works \u{2014} point, scan, confirmed. Also handle manual lookups when needed."}
                        </p>
                        <div style="display:flex;flex-wrap:wrap;gap:0.4rem;">
                            <span style="font-size:0.7rem;color:var(--text-muted);background:var(--bg-tertiary);padding:0.15rem 0.5rem;border-radius:9999px;">"QR scanner"</span>
                            <span style="font-size:0.7rem;color:var(--text-muted);background:var(--bg-tertiary);padding:0.15rem 0.5rem;border-radius:9999px;">"Instant confirm"</span>
                            <span style="font-size:0.7rem;color:var(--text-muted);background:var(--bg-tertiary);padding:0.15rem 0.5rem;border-radius:9999px;">"Manual search"</span>
                        </div>
                    </div>
                </div>
            </button>

            // ── Attendee Card ──
            <button
                style="width:100%;text-align:left;background:var(--bg-secondary);border:1px solid var(--border);border-radius:var(--radius);padding:1.5rem;cursor:pointer;transition:border-color 0.2s;"
                on:click={
                    let on_pick = on_pick.clone();
                    move |_| on_pick(DemoRole::Attendee)
                }
            >
                <div style="display:flex;align-items:flex-start;gap:1rem;">
                    <div style="width:52px;height:52px;border-radius:12px;background:rgba(34,197,94,0.15);border:1px solid rgba(34,197,94,0.3);display:flex;align-items:center;justify-content:center;font-size:1.5rem;flex-shrink:0;">
                        {"\u{1f3ab}"}
                    </div>
                    <div style="flex:1;min-width:0;">
                        <div style="display:flex;align-items:center;gap:0.5rem;margin-bottom:0.25rem;">
                            <span style="font-size:1.1rem;font-weight:700;color:#fff;">"Attendee"</span>
                            <span style="font-size:0.7rem;padding:0.15rem 0.5rem;background:rgba(34,197,94,0.15);color:#22c55e;border:1px solid rgba(34,197,94,0.3);border-radius:9999px;">"Shows up & earns"</span>
                        </div>
                        <p style="font-size:0.85rem;color:var(--text-secondary);line-height:1.5;margin-bottom:0.5rem;">
                            "You're Alex, a developer who registered with a deposit. Walk through getting scanned, taking the quiz, claiming your NFT badge, and getting your money back."
                        </p>
                        <div style="display:flex;flex-wrap:wrap;gap:0.4rem;">
                            <span style="font-size:0.7rem;color:var(--text-muted);background:var(--bg-tertiary);padding:0.15rem 0.5rem;border-radius:9999px;">"Get scanned"</span>
                            <span style="font-size:0.7rem;color:var(--text-muted);background:var(--bg-tertiary);padding:0.15rem 0.5rem;border-radius:9999px;">"Quiz"</span>
                            <span style="font-size:0.7rem;color:var(--text-muted);background:var(--bg-tertiary);padding:0.15rem 0.5rem;border-radius:9999px;">"Claim NFT + refund"</span>
                        </div>
                    </div>
                </div>
            </button>

        </div>

        // Bottom hint
        <div style="text-align:center;margin-top:2rem;">
            <p style="font-size:0.8rem;color:var(--text-muted);">
                "All interactions are simulated. No real data or transactions."
            </p>
        </div>
    }
}

// ── Shared: Persona Header ─────────────────────────────────────────────────────

fn persona_header(role: ReadSignal<DemoRole>) -> impl IntoView {
    view! {
        <div style="display:flex;align-items:center;gap:1rem;margin-bottom:1.5rem;padding:1rem;background:var(--bg-secondary);border:1px solid var(--border);border-radius:var(--radius);">
            <div style=move || format!(
                "width:44px;height:44px;border-radius:50%;background:{};display:flex;align-items:center;justify-content:center;font-size:0.85rem;font-weight:600;color:#fff;flex-shrink:0;border:1px solid {};",
                role.get().accent_color(),
                role.get().accent_border(),
            )>
                {move || role.get().persona_initials()}
            </div>
            <div style="flex:1;min-width:0;">
                <div style="font-weight:600;color:#fff;font-size:0.9rem;">
                    {move || role.get().persona_name()}
                </div>
                <div style="font-size:0.8rem;color:var(--text-secondary);">
                    {move || role.get().persona_title()}
                </div>
            </div>
            <div style=move || format!(
                "font-size:1.5rem;width:40px;height:40px;display:flex;align-items:center;justify-content:center;background:{};border-radius:8px;border:1px solid {};",
                role.get().accent_bg(),
                role.get().accent_border(),
            )>
                {move || role.get().emoji()}
            </div>
        </div>
    }
}

// ── Shared: Step indicator ─────────────────────────────────────────────────────

fn step_indicator(current: usize, total: usize, accent: &'static str) -> impl IntoView {
    view! {
        <div style="display:flex;justify-content:center;gap:0.5rem;margin-bottom:1.5rem;">
            {(0..total).map(|i| {
                let active = i <= current;
                let done = i < current;
                view! {
                    <div style=move || format!(
                        "width:28px;height:28px;border-radius:50%;display:inline-flex;align-items:center;justify-content:center;font-size:0.75rem;font-weight:600;transition:all 0.3s ease;border:1px solid {};background:{};color:{};",
                        if active { accent } else { "var(--border)" }.to_string(),
                        if done { accent.to_string() } else if active { "rgba(99,102,241,0.1)".to_string() } else { "transparent".to_string() },
                        if done { "#fff".to_string() } else if active { "#fff".to_string() } else { "var(--text-muted)".to_string() },
                    )>
                        {if done { "\u{2713}".to_string() } else { format!("{}", i + 1) }}
                    </div>
                }
            }).collect::<Vec<_>>()}
        </div>
    }
}

// ── Shared: Completion card ────────────────────────────────────────────────────

fn completion_card(
    role: DemoRole,
    summary: &'static str,
    highlight_label: &'static str,
    highlight_value: &'static str,
) -> impl IntoView {
    view! {
        <div class="card" style="text-align:center;padding:2rem;">
            // Success icon
            <div style=move || format!(
                "width:64px;height:64px;border-radius:50%;background:{};display:inline-flex;align-items:center;justify-content:center;margin-bottom:1rem;",
                role.accent_bg(),
            )>
                <svg width="28" height="28" viewBox="0 0 24 24" fill="none" stroke="{role.accent_color()}" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round">
                    <polyline points="20 6 9 17 4 12"></polyline>
                </svg>
            </div>

            <h2 style="font-size:1.2rem;font-weight:700;color:#fff;margin-bottom:0.5rem;">
                {role.title()} " journey complete!"
            </h2>
            <p style="font-size:0.85rem;color:var(--text-secondary);line-height:1.6;margin-bottom:1.25rem;">
                {summary}
            </p>

            // Highlight stat
            <div style=move || format!(
                "background:{};border:1px solid {};border-radius:var(--radius);padding:1rem;margin-bottom:1.5rem;",
                role.accent_bg(),
                role.accent_border(),
            )>
                <div style="font-size:0.8rem;color:var(--text-secondary);">{highlight_label}</div>
                <div style=move || format!("font-weight:700;font-size:1.1rem;color:{};", role.accent_color())>
                    {highlight_value}
                </div>
            </div>

            // CTAs
            <div style="display:flex;flex-direction:column;gap:0.75rem;">
                <A href="/login" attr:class="btn btn-primary" attr:style="width:100%;padding:0.85rem;font-size:0.95rem;">
                    {"Start Your Own Event \u{2014} Free"}
                </A>
                <A href="/" attr:class="btn btn-outline" attr:style="width:100%;">
                    "Back to Home"
                </A>
            </div>

            <p style="font-size:0.75rem;color:var(--text-muted);margin-top:1rem;">
                "This was a simulated demo. In production, all interactions use real Solana transactions."
            </p>
        </div>
    }
}

// ── Shared: Next step button ───────────────────────────────────────────────────

fn next_button(label: &'static str, on_click: impl Fn() + 'static) -> impl IntoView {
    view! {
        <button class="btn btn-primary" style="width:100%;padding:0.85rem;font-size:0.95rem;" on:click=move |_| on_click()>
            {label}
        </button>
    }
}

// ── Organizer Journey ──────────────────────────────────────────────────────────

fn organizer_journey(
    role: ReadSignal<DemoRole>,
    step: ReadSignal<OrganizerStep>,
    set_step: WriteSignal<OrganizerStep>,
    simulating: ReadSignal<bool>,
    set_simulating: WriteSignal<bool>,
) -> impl IntoView {
    let step_index = move || match step.get() {
        OrganizerStep::Setup => 0,
        OrganizerStep::Dashboard => 1,
        OrganizerStep::LiveCheckin => 2,
        OrganizerStep::Payout => 3,
        OrganizerStep::Done => 4,
    };

    let go_dashboard = move || {
        set_simulating.set(true);
        set_step.set(OrganizerStep::Dashboard);
        leptos::task::spawn_local(async move {
            gloo::timers::future::TimeoutFuture::new(1000).await;
            set_simulating.set(false);
        });
    };

    let go_live = move || {
        set_simulating.set(true);
        set_step.set(OrganizerStep::LiveCheckin);
        leptos::task::spawn_local(async move {
            gloo::timers::future::TimeoutFuture::new(1200).await;
            set_simulating.set(false);
        });
    };

    let go_payout = move || {
        set_simulating.set(true);
        set_step.set(OrganizerStep::Payout);
        leptos::task::spawn_local(async move {
            gloo::timers::future::TimeoutFuture::new(1000).await;
            set_simulating.set(false);
        });
    };

    let go_done = move || {
        set_step.set(OrganizerStep::Done);
    };

    view! {
        {persona_header(role)}

        <p style="font-size:0.85rem;color:var(--text-secondary);line-height:1.6;margin-bottom:1.5rem;text-align:center;">
            {DemoRole::Organizer.persona_goal()}
        </p>

        // Event context
        <div style="background:var(--bg-secondary);border:1px solid var(--border);border-radius:var(--radius);padding:0.75rem 1rem;margin-bottom:1.5rem;display:flex;align-items:center;gap:0.75rem;">
            <span style="font-size:1.25rem;">{"\u{1f3aa}"}</span>
            <div>
                <div style="font-weight:600;color:#fff;font-size:0.85rem;">{DemoEvent::name()}</div>
                <div style="font-size:0.75rem;color:var(--text-muted);">{DemoEvent::date()} " \u{b7} " {DemoEvent::location()}</div>
            </div>
        </div>

        {step_indicator(step_index(), 5, DemoRole::Organizer.accent_color())}

        // ── Step 0: Event Setup ──
        <Show when=move || step.get() == OrganizerStep::Setup>
            <div class="card" style="padding:1.75rem;">
                <div style="display:flex;align-items:center;gap:0.5rem;margin-bottom:1rem;">
                    <span style="font-size:1.1rem;">{"\u{1f4cb}"}</span>
                    <h2 style="font-size:1rem;font-weight:700;color:#fff;">"Create Event & Set Deposit"</h2>
                </div>
                <p style="font-size:0.85rem;color:var(--text-secondary);line-height:1.5;margin-bottom:1.25rem;">
                    {"Set up your event details and choose the deposit amount. This is what attendees commit \u{2014} and what you keep if they don't show up."}
                </p>

                // Mock form
                <div style="display:flex;flex-direction:column;gap:0.75rem;margin-bottom:1.25rem;">
                    <div style="background:var(--bg-tertiary);border:1px solid var(--border);border-radius:var(--radius-sm);padding:0.65rem 0.85rem;">
                        <div style="font-size:0.7rem;color:var(--text-muted);margin-bottom:0.15rem;">"Event Name"</div>
                        <div style="font-size:0.85rem;color:#fff;">{DemoEvent::name()}</div>
                    </div>
                    <div style="display:grid;grid-template-columns:1fr 1fr;gap:0.5rem;">
                        <div style="background:var(--bg-tertiary);border:1px solid var(--border);border-radius:var(--radius-sm);padding:0.65rem 0.85rem;">
                            <div style="font-size:0.7rem;color:var(--text-muted);margin-bottom:0.15rem;">"Capacity"</div>
                            <div style="font-size:0.85rem;color:#fff;">{format!("{}", DemoEvent::capacity())}</div>
                        </div>
                        <div style="background:var(--bg-tertiary);border:1px solid var(--border);border-radius:var(--radius-sm);padding:0.65rem 0.85rem;">
                            <div style="font-size:0.7rem;color:var(--text-muted);margin-bottom:0.15rem;">"Deposit"</div>
                            <div style="font-size:0.85rem;color:#fff;">{DemoEvent::deposit()}</div>
                        </div>
                    </div>
                </div>

                <div style="background:rgba(99,102,241,0.08);border:1px solid rgba(99,102,241,0.2);border-radius:var(--radius);padding:0.75rem;margin-bottom:1.25rem;">
                    <div style="font-size:0.8rem;color:var(--accent);">
                        {"\u{1f4a1} With 200 attendees at this deposit, no-shows would have cost you real money. BeThere changes that."}
                    </div>
                </div>

                {next_button("Create Event \u{2192}", go_dashboard)}
            </div>
        </Show>

        // ── Step 1: Dashboard ──
        <Show when=move || step.get() == OrganizerStep::Dashboard && !simulating.get()>
            <div class="card" style="padding:1.75rem;">
                <div style="display:flex;align-items:center;gap:0.5rem;margin-bottom:1rem;">
                    <span style="font-size:1.1rem;">{"\u{1f4ca}"}</span>
                    <h2 style="font-size:1rem;font-weight:700;color:#fff;">"Your Dashboard"</h2>
                </div>
                <p style="font-size:0.85rem;color:var(--text-secondary);line-height:1.5;margin-bottom:1.25rem;">
                    "Real-time view of your event. See who registered, who checked in, and how much deposit is at stake."
                </p>

                // Mock dashboard stats
                <div style="display:grid;grid-template-columns:repeat(3,1fr);gap:0.5rem;margin-bottom:1rem;">
                    <div style="background:var(--bg-tertiary);border:1px solid var(--border);border-radius:var(--radius-sm);padding:0.75rem;text-align:center;">
                        <div style="font-weight:700;color:#fff;font-size:1.1rem;">"147"</div>
                        <div style="font-size:0.7rem;color:var(--text-muted);">"Registered"</div>
                    </div>
                    <div style="background:var(--bg-tertiary);border:1px solid var(--border);border-radius:var(--radius-sm);padding:0.75rem;text-align:center;">
                        <div style="font-weight:700;color:var(--success);font-size:1.1rem;">"89"</div>
                        <div style="font-size:0.7rem;color:var(--text-muted);">"Checked In"</div>
                    </div>
                    <div style="background:var(--bg-tertiary);border:1px solid var(--border);border-radius:var(--radius-sm);padding:0.75rem;text-align:center;">
                        <div style="font-weight:700;color:var(--warning);font-size:1.1rem;">"58"</div>
                        <div style="font-size:0.7rem;color:var(--text-muted);">"Pending"</div>
                    </div>
                </div>

                // Mock progress bar
                <div style="margin-bottom:1.25rem;">
                    <div style="display:flex;justify-content:space-between;margin-bottom:0.25rem;">
                        <span style="font-size:0.75rem;color:var(--text-muted);">"Check-in Progress"</span>
                        <span style="font-size:0.75rem;color:var(--accent);">"60%"</span>
                    </div>
                    <div style="height:6px;background:var(--bg-tertiary);border-radius:9999px;overflow:hidden;">
                        <div style="width:60%;height:100%;background:linear-gradient(90deg,#6366f1,#a78bfa);border-radius:9999px;"></div>
                    </div>
                </div>

                {next_button("Event Day: Watch Check-ins \u{2192}", go_live)}
            </div>
        </Show>

        // ── Step 2: Live Check-in ──
        <Show when=move || step.get() == OrganizerStep::LiveCheckin && !simulating.get()>
            <div class="card" style="padding:1.75rem;">
                <div style="display:flex;align-items:center;gap:0.5rem;margin-bottom:1rem;">
                    <span style="font-size:1.1rem;">{"\u{26a1}"}</span>
                    <h2 style="font-size:1rem;font-weight:700;color:#fff;">"Live: Check-ins Rolling In"</h2>
                </div>
                <p style="font-size:0.85rem;color:var(--text-secondary);line-height:1.5;margin-bottom:1.25rem;">
                    "Staff are scanning QR codes at the door. Each scan confirms attendance and triggers the refund process. You see everything in real-time."
                </p>

                // Mock live feed
                <div style="display:flex;flex-direction:column;gap:0.5rem;margin-bottom:1.25rem;">
                    {vec![
                        ("Alex C.", "\u{2713} Checked in", "2 min ago"),
                        ("Sarah K.", "\u{2713} Checked in", "3 min ago"),
                        ("Mike T.", "\u{2713} Checked in", "5 min ago"),
                        ("Jamie L.", "\u{2717} No show", "Event started"),
                    ].into_iter().map(|(name, status, time)| {
                        let is_success = status.starts_with("\u{2713}");
                        view! {
                            <div style="display:flex;align-items:center;justify-content:space-between;padding:0.5rem 0.75rem;background:var(--bg-tertiary);border:1px solid var(--border);border-radius:var(--radius-sm);">
                                <div style="display:flex;align-items:center;gap:0.5rem;">
                                    <div style=move || format!(
                                        "width:24px;height:24px;border-radius:50%;background:{};display:flex;align-items:center;justify-content:center;font-size:0.55rem;font-weight:600;color:#fff;",
                                        if is_success { "rgba(34,197,94,0.3)" } else { "rgba(245,158,11,0.3)" },
                                    )>
                                        {name.chars().next().unwrap_or('?')}
                                    </div>
                                    <span style="font-size:0.8rem;color:#fff;font-weight:500;">{name}</span>
                                </div>
                                <div style="display:flex;align-items:center;gap:0.75rem;">
                                    <span style=move || format!("font-size:0.75rem;color:{};", if is_success { "var(--success)" } else { "var(--warning)" })>
                                        {status}
                                    </span>
                                    <span style="font-size:0.7rem;color:var(--text-muted);">{time}</span>
                                </div>
                            </div>
                        }
                    }).collect::<Vec<_>>()}
                </div>

                // Updated stats
                <div style="background:rgba(99,102,241,0.08);border:1px solid rgba(99,102,241,0.2);border-radius:var(--radius);padding:0.75rem;margin-bottom:1.25rem;">
                    <div style="font-size:0.8rem;color:var(--accent);">
                        {"\u{1f4b0} 3 no-shows so far = 0.03 SOL + $39 USDC recovered to you"}
                    </div>
                </div>

                {next_button("Event Over: See Payouts \u{2192}", go_payout)}
            </div>
        </Show>

        // ── Step 3: Payout ──
        <Show when=move || step.get() == OrganizerStep::Payout && !simulating.get()>
            <div class="card" style="padding:1.75rem;">
                <div style="display:flex;align-items:center;gap:0.5rem;margin-bottom:1rem;">
                    <span style="font-size:1.1rem;">{"\u{1f4b0}"}</span>
                    <h2 style="font-size:1rem;font-weight:700;color:#fff;">{"Event Complete \u{2014} Payouts"}</h2>
                </div>
                <p style="font-size:0.85rem;color:var(--text-secondary);line-height:1.5;margin-bottom:1.25rem;">
                    "The event is over. BeThere automatically refunds all checked-in attendees and sends you the no-show deposits."
                </p>

                // Payout summary
                <div style="display:grid;grid-template-columns:1fr 1fr;gap:0.75rem;margin-bottom:1.25rem;">
                    <div style="background:rgba(34,197,94,0.08);border:1px solid rgba(34,197,94,0.2);border-radius:var(--radius);padding:1rem;text-align:center;">
                        <div style="font-size:0.75rem;color:var(--text-muted);margin-bottom:0.25rem;">"Refunded"</div>
                        <div style="font-weight:700;color:var(--success);font-size:1rem;">"144 attendees"</div>
                        <div style="font-size:0.7rem;color:var(--text-muted);margin-top:0.25rem;">"1.44 SOL + $1,872"</div>
                    </div>
                    <div style="background:rgba(99,102,241,0.08);border:1px solid rgba(99,102,241,0.2);border-radius:var(--radius);padding:1rem;text-align:center;">
                        <div style="font-size:0.75rem;color:var(--text-muted);margin-bottom:0.25rem;">"You received"</div>
                        <div style="font-weight:700;color:var(--accent);font-size:1rem;">"3 no-shows"</div>
                        <div style="font-size:0.7rem;color:var(--text-muted);margin-top:0.25rem;">"0.03 SOL + $39"</div>
                    </div>
                </div>

                <div style="background:var(--bg-tertiary);border:1px solid var(--border);border-radius:var(--radius);padding:0.75rem;margin-bottom:1.25rem;">
                    <div style="font-size:0.8rem;color:var(--text-secondary);">
                        {"\u{2705} 144 NFT badges minted on Solana (cNFT) \u{2014} cost: ~$0.14 total"}
                    </div>
                </div>

                {next_button("See Summary \u{2192}", go_done)}
            </div>
        </Show>

        // ── Step 4: Done ──
        <Show when=move || step.get() == OrganizerStep::Done>
            {completion_card(
                DemoRole::Organizer,
                "You created an event, set deposits to prevent no-shows, tracked everything in real-time, and collected funds from people who didn't show up. All automated \u{2014} no spreadsheets needed.",
                "No-show revenue recovered",
                "0.03 SOL + $39 USDC",
            )}
        </Show>
    }
}

// ── Staff Journey ──────────────────────────────────────────────────────────────

fn staff_journey(
    role: ReadSignal<DemoRole>,
    step: ReadSignal<StaffStep>,
    set_step: WriteSignal<StaffStep>,
    simulating: ReadSignal<bool>,
    set_simulating: WriteSignal<bool>,
) -> impl IntoView {
    let step_index = move || match step.get() {
        StaffStep::Scan => 0,
        StaffStep::Confirmed => 1,
        StaffStep::Manual => 2,
        StaffStep::Done => 3,
    };

    let go_confirmed = move || {
        set_simulating.set(true);
        set_step.set(StaffStep::Confirmed);
        leptos::task::spawn_local(async move {
            gloo::timers::future::TimeoutFuture::new(1500).await;
            set_simulating.set(false);
        });
    };

    let go_manual = move || {
        set_simulating.set(true);
        set_step.set(StaffStep::Manual);
        leptos::task::spawn_local(async move {
            gloo::timers::future::TimeoutFuture::new(800).await;
            set_simulating.set(false);
        });
    };

    let go_done = move || {
        set_step.set(StaffStep::Done);
    };

    view! {
        {persona_header(role)}

        <p style="font-size:0.85rem;color:var(--text-secondary);line-height:1.6;margin-bottom:1.5rem;text-align:center;">
            {DemoRole::Staff.persona_goal()}
        </p>

        // Event context
        <div style="background:var(--bg-secondary);border:1px solid var(--border);border-radius:var(--radius);padding:0.75rem 1rem;margin-bottom:1.5rem;display:flex;align-items:center;gap:0.75rem;">
            <span style="font-size:1.25rem;">{"\u{1f3aa}"}</span>
            <div>
                <div style="font-weight:600;color:#fff;font-size:0.85rem;">{DemoEvent::name()}</div>
                <div style="font-size:0.75rem;color:var(--text-muted);">"Your station: Door A"</div>
            </div>
        </div>

        {step_indicator(step_index(), 4, DemoRole::Staff.accent_color())}

        // ── Step 0: Scan ──
        <Show when=move || step.get() == StaffStep::Scan && !simulating.get()>
            <div class="card" style="text-align:center;padding:1.75rem;">
                <div style="display:flex;align-items:center;gap:0.5rem;margin-bottom:1rem;justify-content:center;">
                    <span style="font-size:1.1rem;">{"\u{1f4f7}"}</span>
                    <h2 style="font-size:1rem;font-weight:700;color:#fff;">"Scan Attendee QR Code"</h2>
                </div>
                <p style="font-size:0.85rem;color:var(--text-secondary);line-height:1.5;margin-bottom:1.25rem;">
                    "An attendee approaches. Point your phone at their QR code. The app instantly recognizes them."
                </p>

                // Mock scanner view
                <div style="width:200px;height:200px;margin:0 auto 1.25rem;border:2px solid var(--accent);border-radius:12px;overflow:hidden;display:flex;align-items:center;justify-content:center;background:var(--bg-tertiary);position:relative;">
                    <div style="position:absolute;top:0;left:0;right:0;height:2px;background:linear-gradient(90deg,transparent,var(--accent),transparent);animation:scan-line 1.5s ease-in-out infinite;"></div>
                    <div style="color:var(--text-muted);font-size:0.8rem;">"Point at QR..."</div>
                </div>

                // Simulated attendee approaching
                <div style="background:var(--bg-tertiary);border:1px solid var(--border);border-radius:var(--radius);padding:0.75rem;margin-bottom:1.25rem;display:flex;align-items:center;gap:0.75rem;">
                    <div style="width:32px;height:32px;border-radius:50%;background:rgba(34,197,94,0.2);display:flex;align-items:center;justify-content:center;font-size:0.7rem;font-weight:600;color:var(--success);">
                        "AC"
                    </div>
                    <div style="text-align:left;">
                        <div style="font-size:0.8rem;color:#fff;font-weight:500;">"Alex Chen"</div>
                        <div style="font-size:0.7rem;color:var(--text-muted);">{"General Admission \u{b7} QR ready"}</div>
                    </div>
                </div>

                {next_button("Simulate: Scan QR Code", go_confirmed)}
            </div>
        </Show>

        // ── Step 1: Confirmed ──
        <Show when=move || step.get() == StaffStep::Confirmed && !simulating.get()>
            <div class="card" style="padding:1.75rem;">
                // Success flash
                <div style="text-align:center;margin-bottom:1rem;">
                    <div style="width:48px;height:48px;border-radius:50%;background:rgba(34,197,94,0.15);display:inline-flex;align-items:center;justify-content:center;border:1px solid rgba(34,197,94,0.3);">
                        <svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="var(--success)" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round">
                            <polyline points="20 6 9 17 4 12"></polyline>
                        </svg>
                    </div>
                </div>

                <h2 style="font-size:1rem;font-weight:700;color:#fff;text-align:center;margin-bottom:0.25rem;">"Checked In!"</h2>
                <p style="font-size:0.85rem;color:var(--text-secondary);text-align:center;margin-bottom:1.25rem;">
                    "Scanned and confirmed in under 2 seconds. Alex can now head inside."
                </p>

                // Attendee detail
                <div style="background:var(--bg-tertiary);border:1px solid var(--border);border-radius:var(--radius);padding:1rem;margin-bottom:1.25rem;">
                    <div style="display:flex;align-items:center;gap:0.75rem;margin-bottom:0.75rem;">
                        <div style="width:36px;height:36px;border-radius:50%;background:rgba(34,197,94,0.2);display:flex;align-items:center;justify-content:center;font-size:0.75rem;font-weight:600;color:var(--success);">
                            "AC"
                        </div>
                        <div>
                            <div style="font-weight:600;color:#fff;font-size:0.9rem;">"Alex Chen"</div>
                            <div style="font-size:0.75rem;color:var(--text-muted);">"alex@dev.email"</div>
                        </div>
                    </div>
                    <div style="display:flex;gap:0.5rem;">
                        <span style="font-size:0.7rem;padding:0.15rem 0.5rem;background:rgba(34,197,94,0.15);color:var(--success);border-radius:9999px;border:1px solid rgba(34,197,94,0.3);">{"\u{2713} Checked in"}</span>
                        <span style="font-size:0.7rem;padding:0.15rem 0.5rem;background:var(--bg-secondary);color:var(--text-muted);border-radius:9999px;border:1px solid var(--border);">"General Admission"</span>
                    </div>
                </div>

                <div style="background:rgba(245,158,11,0.08);border:1px solid rgba(245,158,11,0.2);border-radius:var(--radius);padding:0.75rem;margin-bottom:1.25rem;">
                    <div style="font-size:0.8rem;color:var(--warning);">
                        {"\u{1f4a1} What if someone lost their QR code? You can search by name or email instead."}
                    </div>
                </div>

                {next_button("Try Manual Lookup \u{2192}", go_manual)}
            </div>
        </Show>

        // ── Step 2: Manual Lookup ──
        <Show when=move || step.get() == StaffStep::Manual && !simulating.get()>
            <div class="card" style="padding:1.75rem;">
                <div style="display:flex;align-items:center;gap:0.5rem;margin-bottom:1rem;">
                    <span style="font-size:1.1rem;">{"\u{1f50d}"}</span>
                    <h2 style="font-size:1rem;font-weight:700;color:#fff;">"Manual Lookup"</h2>
                </div>
                <p style="font-size:0.85rem;color:var(--text-secondary);line-height:1.5;margin-bottom:1.25rem;">
                    "Someone's QR code isn't working? No problem. Search by name or email and check them in manually."
                </p>

                // Mock search
                <div style="margin-bottom:1rem;">
                    <div style="background:var(--bg-tertiary);border:1px solid var(--border);border-radius:var(--radius-sm);padding:0.65rem 0.85rem;">
                        <div style="font-size:0.7rem;color:var(--text-muted);margin-bottom:0.15rem;">"Search attendees..."</div>
                        <div style="font-size:0.85rem;color:#fff;">"Sarah"</div>
                    </div>
                </div>

                // Search results
                <div style="display:flex;flex-direction:column;gap:0.5rem;margin-bottom:1.25rem;">
                    <div style="background:var(--bg-tertiary);border:1px solid var(--border);border-radius:var(--radius-sm);padding:0.75rem;display:flex;align-items:center;justify-content:space-between;">
                        <div style="display:flex;align-items:center;gap:0.5rem;">
                            <div style="width:28px;height:28px;border-radius:50%;background:rgba(99,102,241,0.2);display:flex;align-items:center;justify-content:center;font-size:0.6rem;font-weight:600;color:var(--accent);">
                                "SK"
                            </div>
                            <div>
                                <div style="font-size:0.85rem;color:#fff;font-weight:500;">"Sarah K."</div>
                                <div style="font-size:0.7rem;color:var(--text-muted);">"sarah@web3.dev"</div>
                            </div>
                        </div>
                        <span style="font-size:0.7rem;padding:0.15rem 0.5rem;background:rgba(34,197,94,0.15);color:var(--success);border-radius:9999px;border:1px solid rgba(34,197,94,0.3);">"Check In"</span>
                    </div>
                </div>

                {next_button("See Summary \u{2192}", go_done)}
            </div>
        </Show>

        // ── Step 3: Done ──
        <Show when=move || step.get() == StaffStep::Done>
            {completion_card(
                DemoRole::Staff,
                    "You checked in attendees by scanning QR codes \u{2014} each one confirmed in under 2 seconds. When someone lost their code, you found them instantly with a manual search. No paper lists, no confusion.",
                    "Avg. check-in time",
                "< 2 seconds per person",
            )}
        </Show>
    }
}

// ── Attendee Journey ───────────────────────────────────────────────────────────

fn attendee_journey(
    role: ReadSignal<DemoRole>,
    step: ReadSignal<AttendeeStep>,
    set_step: WriteSignal<AttendeeStep>,
    simulating: ReadSignal<bool>,
    set_simulating: WriteSignal<bool>,
    quiz_answer: ReadSignal<Option<usize>>,
    set_quiz_answer: WriteSignal<Option<usize>>,
    quiz_submitted: ReadSignal<bool>,
    set_quiz_submitted: WriteSignal<bool>,
) -> impl IntoView {
    let step_index = move || match step.get() {
        AttendeeStep::Arrive => 0,
        AttendeeStep::Scanned => 1,
        AttendeeStep::Quiz => 2,
        AttendeeStep::Claim => 3,
        AttendeeStep::Done => 4,
    };

    let go_scanned = move || {
        set_simulating.set(true);
        set_step.set(AttendeeStep::Scanned);
        leptos::task::spawn_local(async move {
            gloo::timers::future::TimeoutFuture::new(1500).await;
            set_simulating.set(false);
        });
    };

    let go_quiz = move || {
        set_step.set(AttendeeStep::Quiz);
    };

    let go_claim = move || {
        set_simulating.set(true);
        set_step.set(AttendeeStep::Claim);
        leptos::task::spawn_local(async move {
            gloo::timers::future::TimeoutFuture::new(2000).await;
            set_simulating.set(false);
            set_step.set(AttendeeStep::Done);
        });
    };

    let submit_quiz = move || {
        if quiz_answer.get().is_some() {
            set_quiz_submitted.set(true);
        }
    };

    view! {
        {persona_header(role)}

        <p style="font-size:0.85rem;color:var(--text-secondary);line-height:1.6;margin-bottom:1.5rem;text-align:center;">
            {DemoRole::Attendee.persona_goal()}
        </p>

        // Event + deposit context
        <div style="background:var(--bg-secondary);border:1px solid var(--border);border-radius:var(--radius);padding:0.75rem 1rem;margin-bottom:1.5rem;display:flex;align-items:center;justify-content:space-between;">
            <div style="display:flex;align-items:center;gap:0.75rem;">
                <span style="font-size:1.25rem;">{"\u{1f3aa}"}</span>
                <div>
                    <div style="font-weight:600;color:#fff;font-size:0.85rem;">{DemoEvent::name()}</div>
                    <div style="font-size:0.75rem;color:var(--text-muted);">{DemoEvent::date()} " \u{b7} " {DemoEvent::location()}</div>
                </div>
            </div>
            <div style="text-align:right;">
                <div style="font-size:0.7rem;color:var(--text-muted);">"Your deposit"</div>
                <div style="font-size:0.8rem;color:var(--accent);font-weight:600;">{DemoEvent::deposit()}</div>
            </div>
        </div>

        {step_indicator(step_index(), 5, DemoRole::Attendee.accent_color())}

        // ── Step 0: Arrive ──
        <Show when=move || step.get() == AttendeeStep::Arrive>
            <div class="card" style="text-align:center;padding:1.75rem;">
                <div style="width:56px;height:56px;border-radius:12px;background:rgba(34,197,94,0.15);border:1px solid rgba(34,197,94,0.3);display:inline-flex;align-items:center;justify-content:center;font-size:1.5rem;margin-bottom:1rem;">{"\u{1f3ab}"}</div>
                <h2 style="font-size:1.1rem;font-weight:700;color:#fff;margin-bottom:0.5rem;">
                    "You're at the venue!"
                </h2>
                <p style="font-size:0.85rem;color:var(--text-secondary);line-height:1.5;margin-bottom:1.25rem;">
                    "You registered with a deposit. Now show your QR code to the staff at the door. They'll scan it and confirm your check-in."
                </p>

                // Mock QR code
                <div style="width:160px;height:160px;margin:0 auto 1.25rem;background:#fff;border-radius:8px;display:flex;align-items:center;justify-content:center;border:2px solid rgba(255,255,255,0.1);">
                    <div style="color:#000;font-size:0.7rem;font-weight:600;text-align:center;padding:0.5rem;">
                        "[ QR CODE ]\nalex@dev.email\nSolana Bangkok 2025"
                    </div>
                </div>

                {next_button("Staff Scans Your QR \u{2192}", go_scanned)}
            </div>
        </Show>

        // ── Step 1: Scanned ──
        <Show when=move || step.get() == AttendeeStep::Scanned && !simulating.get()>
            <div class="card" style="text-align:center;padding:1.75rem;">
                <div style="width:48px;height:48px;border-radius:50%;background:rgba(34,197,94,0.15);display:inline-flex;align-items:center;justify-content:center;border:1px solid rgba(34,197,94,0.3);margin-bottom:1rem;">
                    <svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="var(--success)" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round">
                        <polyline points="20 6 9 17 4 12"></polyline>
                    </svg>
                </div>
                <h2 style="font-size:1.1rem;font-weight:700;color:#fff;margin-bottom:0.25rem;">
                    "You're checked in!"
                </h2>
                <p style="font-size:0.85rem;color:var(--text-secondary);margin-bottom:1.25rem;">
                    "Staff scanned your code. You're confirmed. Now complete a quick quiz to prove you paid attention."
                </p>

                // Check-in confirmation
                <div style="background:rgba(34,197,94,0.08);border:1px solid rgba(34,197,94,0.2);border-radius:var(--radius);padding:0.75rem;margin-bottom:1.25rem;">
                    <div style="font-size:0.8rem;color:var(--success);font-weight:500;">
                        {"\u{2705} Alex Chen \u{2014} checked in at 6:32 PM"}
                    </div>
                </div>

                {next_button("Take the Quiz \u{2192}", go_quiz)}
            </div>
        </Show>

        // ── Step 2: Quiz ──
        <Show when=move || step.get() == AttendeeStep::Quiz && !quiz_submitted.get()>
            <div class="card" style="padding:1.75rem;">
                <div style="display:flex;align-items:center;gap:0.5rem;margin-bottom:1rem;">
                    <span style="font-size:1.1rem;">{"\u{1f9e0}"}</span>
                    <h2 style="font-size:1rem;font-weight:700;color:#fff;">"Quick Quiz"</h2>
                </div>
                <p style="font-size:0.85rem;color:var(--text-secondary);line-height:1.5;margin-bottom:1.25rem;">
                    "Prove you were paying attention. Answer correctly to claim your NFT badge and get your deposit back."
                </p>

                <div style="margin-bottom:1.25rem;">
                    <p style="font-size:0.95rem;color:#fff;font-weight:500;margin-bottom:1rem;">
                        {DemoQuiz::question()}
                    </p>
                    <div style="display:flex;flex-direction:column;gap:0.5rem;">
                        {DemoQuiz::options().iter().enumerate().map(|(i, opt)| {
                            let idx = i;
                            view! {
                                <button
                                    class="btn btn-outline"
                                    style="width:100%;text-align:left;justify-content:flex-start;font-size:0.9rem;"
                                    on:click=move |_| set_quiz_answer.set(Some(idx))
                                >
                                    <span style="font-weight:600;margin-right:0.5rem;">
                                        {format!("{}.", (b'A' + i as u8) as char)}
                                    </span>
                                    {*opt}
                                </button>
                            }
                        }).collect::<Vec<_>>()}
                    </div>
                </div>

                <button
                    class="btn btn-primary"
                    style="width:100%;"
                    disabled=move || quiz_answer.get().is_none()
                    on:click=move |_| submit_quiz()
                >
                    "Submit Answer"
                </button>
            </div>
        </Show>

        // ── Step 2b: Quiz passed ──
        <Show when=move || step.get() == AttendeeStep::Quiz && quiz_submitted.get()>
            <div class="card" style="padding:1.75rem;">
                <div style="background:rgba(34,197,94,0.1);border:1px solid rgba(34,197,94,0.3);border-radius:var(--radius);padding:1rem;margin-bottom:1.25rem;">
                    <div style="font-weight:600;color:var(--success);font-size:0.9rem;">
                        {"\u{2713} Quiz passed!"}
                    </div>
                    <div style="font-size:0.8rem;color:var(--text-secondary);margin-top:0.25rem;">
                        "You proved you paid attention. Now claim your NFT badge and deposit refund."
                    </div>
                </div>

                // What you'll get preview
                <div style="background:var(--bg-tertiary);border:1px solid var(--border);border-radius:var(--radius);padding:1rem;margin-bottom:1.25rem;">
                    <div style="font-size:0.8rem;color:var(--text-muted);margin-bottom:0.5rem;">"You'll receive:"</div>
                    <div style="display:flex;flex-direction:column;gap:0.5rem;">
                        <div style="display:flex;align-items:center;gap:0.5rem;">
                            <span style="font-size:0.9rem;">{"\u{1f3ab}"}</span>
                            <span style="font-size:0.85rem;color:#fff;">"Compressed NFT badge (proof of attendance)"</span>
                        </div>
                        <div style="display:flex;align-items:center;gap:0.5rem;">
                            <span style="font-size:0.9rem;">{"\u{1f4b0}"}</span>
                            <span style="font-size:0.85rem;color:var(--success);">"Full deposit refund: " {DemoEvent::deposit()}</span>
                        </div>
                    </div>
                </div>

                {next_button("Claim NFT Badge + Refund \u{2192}", go_claim)}
            </div>
        </Show>

        // ── Step 3: Claiming ──
        <Show when=move || step.get() == AttendeeStep::Claim>
            <div class="card" style="text-align:center;padding:1.75rem;">
                <div style="margin:1rem auto;">
                    <div class="spinner-lg" style="margin:0 auto;"></div>
                </div>
                <h2 style="font-size:1.1rem;font-weight:700;color:#fff;margin-bottom:0.5rem;">
                    "Minting your NFT badge..."
                </h2>
                <p style="font-size:0.85rem;color:var(--text-secondary);">
                    "Compressing NFT on Solana. This takes ~2 seconds in real life too."
                </p>
            </div>
        </Show>

        // ── Step 4: Done ──
        <Show when=move || step.get() == AttendeeStep::Done>
            <div class="card" style="text-align:center;padding:1.75rem;">
                // Success icon
                <div style="width:64px;height:64px;border-radius:50%;background:linear-gradient(135deg,#22c55e,#16a34a);display:inline-flex;align-items:center;justify-content:center;margin-bottom:1rem;">
                    <svg width="32" height="32" viewBox="0 0 24 24" fill="none" stroke="#fff" stroke-width="3" stroke-linecap="round" stroke-linejoin="round">
                        <polyline points="20 6 9 17 4 12"></polyline>
                    </svg>
                </div>

                <h2 style="font-size:1.25rem;font-weight:700;color:#fff;margin-bottom:0.25rem;">
                    "NFT Badge Claimed!"
                </h2>
                <p style="font-size:0.85rem;color:var(--text-secondary);margin-bottom:1.25rem;">
                    "You proved you were there. Your deposit is refunded. Badge is yours forever."
                </p>

                // Mock NFT card
                <div style="background:var(--bg-tertiary);border:1px solid var(--border);border-radius:var(--radius);overflow:hidden;margin-bottom:1.25rem;text-align:left;">
                    <div style="height:120px;background:linear-gradient(135deg,#22c55e,#16a34a,#4ade80);display:flex;align-items:center;justify-content:center;">
                        <span style="font-size:2.5rem;">{"\u{1f3ab}"}</span>
                    </div>
                    <div style="padding:1rem;">
                        <div style="font-weight:600;color:#fff;font-size:0.9rem;margin-bottom:0.25rem;">
                            "BeThere Proof of Attendance"
                        </div>
                        <div style="font-size:0.8rem;color:var(--text-secondary);margin-bottom:0.5rem;">
                            {DemoEvent::name()}
                        </div>
                        <div style="display:flex;gap:0.5rem;">
                            <span style="font-size:0.7rem;padding:0.2rem 0.5rem;background:rgba(34,197,94,0.15);color:var(--success);border-radius:9999px;border:1px solid rgba(34,197,94,0.3);">
                                "cNFT"
                            </span>
                            <span style="font-size:0.7rem;padding:0.2rem 0.5rem;background:rgba(99,102,241,0.15);color:var(--accent);border-radius:9999px;border:1px solid rgba(99,102,241,0.3);">
                                "Mainnet"
                            </span>
                        </div>
                    </div>
                </div>

                // Deposit refund
                <div style="background:rgba(34,197,94,0.08);border:1px solid rgba(34,197,94,0.2);border-radius:var(--radius);padding:0.75rem 1rem;margin-bottom:1.25rem;">
                    <div style="font-size:0.8rem;color:var(--text-secondary);">
                        "Deposit Refunded"
                    </div>
                    <div style="font-weight:700;color:var(--success);font-size:1.1rem;">
                        {DemoEvent::deposit()}
                    </div>
                </div>

                // CTAs
                <div style="display:flex;flex-direction:column;gap:0.75rem;">
                    <A href="/login" attr:class="btn btn-primary" attr:style="width:100%;padding:0.85rem;font-size:0.95rem;">
                        {"Start Your Own Event \u{2014} Free"}
                    </A>
                    <A href="/" attr:class="btn btn-outline" attr:style="width:100%;">
                        "Back to Home"
                    </A>
                </div>

                <p style="font-size:0.75rem;color:var(--text-muted);margin-top:1rem;">
                    "This was a simulated demo. In production, NFTs are minted on Solana mainnet with real compressed NFTs."
                </p>
            </div>
        </Show>
    }
}
