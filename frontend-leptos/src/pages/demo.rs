//! Interactive role-based demo page — lets visitors experience BeThere from every perspective.

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
            DemoRole::Organizer => "\u{1f3af}",
            DemoRole::Staff => "\u{1f4f1}",
            DemoRole::Attendee => "\u{1f3ab}",
            DemoRole::Picker => "\u{2753}",
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

    fn accent_color(self) -> &'static str {
        match self {
            DemoRole::Organizer => "#6366f1",
            DemoRole::Staff => "#f59e0b",
            DemoRole::Attendee => "#22c55e",
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
    Registrations,
    Dashboard,
    Payout,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum StaffStep {
    Scan,
    Confirmed,
    Done,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum AttendeeStep {
    Register,
    AtVenue,
    Confirmed,
    Quiz,
    Claimed,
}

// ── Demo Data ──────────────────────────────────────────────────────────────────

struct DemoEvent;

impl DemoEvent {
    fn name() -> &'static str { "Solana Bangkok Meetup 2025" }
    fn date() -> &'static str { "Jul 15, 2025" }
    fn location() -> &'static str { "Bangkok, Thailand" }
    fn capacity() -> u32 { 200 }
    fn deposit() -> &'static str { "0.01 SOL + $13 USDC" }
    fn registered() -> u32 { 150 }
    fn checked_in() -> u32 { 142 }
    fn no_shows() -> u32 { 8 }
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

#[component]
pub fn Demo() -> impl IntoView {
    let (role, set_role) = signal(DemoRole::Picker);
    let (org_step, set_org_step) = signal(OrganizerStep::Setup);
    let (staff_step, set_staff_step) = signal(StaffStep::Scan);
    let (attendee_step, set_attendee_step) = signal(AttendeeStep::Register);
    let (simulating, set_simulating) = signal(false);
    let (quiz_answer, set_quiz_answer) = signal(None::<usize>);
    let (quiz_submitted, set_quiz_submitted) = signal(false);

    let pick_role = move |r: DemoRole| {
        set_role.set(r);
        set_org_step.set(OrganizerStep::Setup);
        set_staff_step.set(StaffStep::Scan);
        set_attendee_step.set(AttendeeStep::Register);
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
        <div style="text-align:center;margin-bottom:2rem;">
            <h1 style="font-size:1.5rem;font-weight:800;color:#fff;margin-bottom:0.5rem;">
                "Experience BeThere"
            </h1>
            <p style="font-size:0.9rem;color:var(--text-secondary);">
                "Pick a role to explore the demo."
            </p>
        </div>

        // Compact horizontal role cards
        <div style="display:flex;flex-direction:column;gap:0.75rem;">

            <button
                style="width:100%;text-align:left;background:var(--bg-secondary);border:1px solid var(--border);border-radius:var(--radius);padding:1rem 1.25rem;cursor:pointer;transition:border-color 0.2s;display:flex;align-items:center;gap:1rem;"
                on:click={
                    let on_pick = on_pick.clone();
                    move |_| on_pick(DemoRole::Organizer)
                }
            >
                <div style="width:44px;height:44px;border-radius:10px;background:rgba(99,102,241,0.15);border:1px solid rgba(99,102,241,0.3);display:flex;align-items:center;justify-content:center;font-size:1.25rem;flex-shrink:0;">
                    {"\u{1f3af}"}
                </div>
                <div>
                    <div style="font-weight:700;color:#fff;font-size:0.95rem;">"Organizer"</div>
                    <div style="font-size:0.8rem;color:var(--text-secondary);">"Create events, track attendance, collect no-show funds"</div>
                </div>
            </button>

            <button
                style="width:100%;text-align:left;background:var(--bg-secondary);border:1px solid var(--border);border-radius:var(--radius);padding:1rem 1.25rem;cursor:pointer;transition:border-color 0.2s;display:flex;align-items:center;gap:1rem;"
                on:click={
                    let on_pick = on_pick.clone();
                    move |_| on_pick(DemoRole::Staff)
                }
            >
                <div style="width:44px;height:44px;border-radius:10px;background:rgba(245,158,11,0.15);border:1px solid rgba(245,158,11,0.3);display:flex;align-items:center;justify-content:center;font-size:1.25rem;flex-shrink:0;">
                    {"\u{1f4f1}"}
                </div>
                <div>
                    <div style="font-weight:700;color:#fff;font-size:0.95rem;">"Event Staff"</div>
                    <div style="font-size:0.8rem;color:var(--text-secondary);">"Scan QR codes, confirm check-ins at the door"</div>
                </div>
            </button>

            <button
                style="width:100%;text-align:left;background:var(--bg-secondary);border:1px solid var(--border);border-radius:var(--radius);padding:1rem 1.25rem;cursor:pointer;transition:border-color 0.2s;display:flex;align-items:center;gap:1rem;"
                on:click={
                    let on_pick = on_pick.clone();
                    move |_| on_pick(DemoRole::Attendee)
                }
            >
                <div style="width:44px;height:44px;border-radius:10px;background:rgba(34,197,94,0.15);border:1px solid rgba(34,197,94,0.3);display:flex;align-items:center;justify-content:center;font-size:1.25rem;flex-shrink:0;">
                    {"\u{1f3ab}"}
                </div>
                <div>
                    <div style="font-weight:700;color:#fff;font-size:0.95rem;">"Attendee"</div>
                    <div style="font-size:0.8rem;color:var(--text-secondary);">"Register, check in, earn NFT badge + deposit refund"</div>
                </div>
            </button>

        </div>

        <div style="text-align:center;margin-top:1.5rem;">
            <p style="font-size:0.75rem;color:var(--text-muted);">
                "Simulated demo. No real transactions."
            </p>
        </div>
    }
}

// ── Shared: Persona Header ─────────────────────────────────────────────────────

fn persona_header(role: ReadSignal<DemoRole>) -> impl IntoView {
    view! {
        <div style="display:flex;align-items:center;gap:0.75rem;margin-bottom:1.25rem;padding:0.75rem 1rem;background:var(--bg-secondary);border:1px solid var(--border);border-radius:var(--radius);">
            <div style=move || format!(
                "width:36px;height:36px;border-radius:50%;background:{};display:flex;align-items:center;justify-content:center;font-size:0.75rem;font-weight:600;color:#fff;flex-shrink:0;",
                role.get().accent_color(),
            )>
                {move || role.get().persona_initials()}
            </div>
            <div style="flex:1;min-width:0;">
                <div style="font-weight:600;color:#fff;font-size:0.85rem;">
                    {move || role.get().persona_name()}
                </div>
            </div>
            <div style=move || format!(
                "font-size:1.25rem;width:36px;height:36px;display:flex;align-items:center;justify-content:center;background:{};border-radius:8px;border:1px solid {};",
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

// completion_card removed — all final steps use inline views

// ── Shared: Next step button ───────────────────────────────────────────────────

fn next_button(label: &'static str, on_click: impl Fn() + 'static) -> impl IntoView {
    view! {
        <button class="btn btn-primary" style="width:100%;padding:0.85rem;font-size:0.95rem;" on:click=move |_| on_click()>
            {label}
        </button>
    }
}

// ── Shared: Loading spinner card ───────────────────────────────────────────────

fn loading_card(title: &'static str, subtitle: &'static str) -> impl IntoView {
    view! {
        <div class="card" style="text-align:center;padding:2rem;">
            <div style="margin:1rem auto;">
                <div class="spinner-lg" style="margin:0 auto;"></div>
            </div>
            <h2 style="font-size:1.1rem;font-weight:700;color:#fff;margin-bottom:0.5rem;">
                {title}
            </h2>
            <p style="font-size:0.85rem;color:var(--text-secondary);">
                {subtitle}
            </p>
        </div>
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
        OrganizerStep::Registrations => 1,
        OrganizerStep::Dashboard => 2,
        OrganizerStep::Payout => 3,
    };

    let delay_next = |target: OrganizerStep, ms: u32| {
        move || {
            set_simulating.set(true);
            set_step.set(target);
            leptos::task::spawn_local(async move {
                gloo::timers::future::TimeoutFuture::new(ms).await;
                set_simulating.set(false);
            });
        }
    };

    let go_registrations = delay_next(OrganizerStep::Registrations, 800);
    let go_dashboard = delay_next(OrganizerStep::Dashboard, 800);
    let go_payout = move || {
        set_step.set(OrganizerStep::Payout);
    };

    view! {
        {persona_header(role)}

        // Event context
        <div style="background:var(--bg-secondary);border:1px solid var(--border);border-radius:var(--radius);padding:0.65rem 1rem;margin-bottom:1.25rem;display:flex;align-items:center;gap:0.75rem;">
            <span style="font-size:1.1rem;">{"\u{1f3aa}"}</span>
            <div>
                <div style="font-weight:600;color:#fff;font-size:0.85rem;">{DemoEvent::name()}</div>
                <div style="font-size:0.75rem;color:var(--text-muted);">{DemoEvent::date()} {" \u{b7} "} {DemoEvent::location()}</div>
            </div>
        </div>

        {move || step_indicator(step_index(), 4, DemoRole::Organizer.accent_color())}

        // ── Step 0: Setup ──
        <Show when=move || step.get() == OrganizerStep::Setup>
            <div class="card" style="padding:1.5rem;">
                <div style="display:flex;align-items:center;gap:0.5rem;margin-bottom:1rem;">
                    <span style="font-size:1.1rem;">{"\u{1f4cb}"}</span>
                    <h2 style="font-size:1rem;font-weight:700;color:#fff;">"Create Event"</h2>
                </div>

                // Mock form
                <div style="display:flex;flex-direction:column;gap:0.65rem;margin-bottom:1.25rem;">
                    <div style="background:var(--bg-tertiary);border:1px solid var(--border);border-radius:var(--radius-sm);padding:0.6rem 0.85rem;">
                        <div style="font-size:0.7rem;color:var(--text-muted);margin-bottom:0.15rem;">"Event Name"</div>
                        <div style="font-size:0.85rem;color:#fff;">{DemoEvent::name()}</div>
                    </div>
                    <div style="display:grid;grid-template-columns:1fr 1fr;gap:0.5rem;">
                        <div style="background:var(--bg-tertiary);border:1px solid var(--border);border-radius:var(--radius-sm);padding:0.6rem 0.85rem;">
                            <div style="font-size:0.7rem;color:var(--text-muted);margin-bottom:0.15rem;">"Capacity"</div>
                            <div style="font-size:0.85rem;color:#fff;">{format!("{}", DemoEvent::capacity())}</div>
                        </div>
                        <div style="background:var(--bg-tertiary);border:1px solid var(--border);border-radius:var(--radius-sm);padding:0.6rem 0.85rem;">
                            <div style="font-size:0.7rem;color:var(--text-muted);margin-bottom:0.15rem;">"Deposit"</div>
                            <div style="font-size:0.85rem;color:#fff;">{DemoEvent::deposit()}</div>
                        </div>
                    </div>
                </div>

                {next_button("Create Event \u{2192}", go_registrations)}
            </div>
        </Show>

        // ── Step 1: Registrations (loading → content) ──
        <Show when=move || step.get() == OrganizerStep::Registrations && simulating.get()>
            {loading_card("Creating event...", "Setting up deposit pool")}
        </Show>

        <Show when=move || step.get() == OrganizerStep::Registrations && !simulating.get()>
            <div class="card" style="padding:1.5rem;">
                <div style="display:flex;align-items:center;gap:0.5rem;margin-bottom:1rem;">
                    <span style="font-size:1.1rem;">{"\u{1f4e5}"}</span>
                    <h2 style="font-size:1rem;font-weight:700;color:#fff;">"Registrations"</h2>
                </div>

                // Registration stats
                <div style="display:grid;grid-template-columns:1fr 1fr;gap:0.5rem;margin-bottom:1rem;">
                    <div style="background:var(--bg-tertiary);border:1px solid var(--border);border-radius:var(--radius-sm);padding:0.75rem;text-align:center;">
                        <div style="font-weight:700;color:#fff;font-size:1.1rem;">{format!("{}", DemoEvent::registered())}</div>
                        <div style="font-size:0.7rem;color:var(--text-muted);">"Registered"</div>
                    </div>
                    <div style="background:var(--bg-tertiary);border:1px solid var(--border);border-radius:var(--radius-sm);padding:0.75rem;text-align:center;">
                        <div style="font-weight:700;color:#6366f1;font-size:1.1rem;">"1.5 SOL + $1,950"</div>
                        <div style="font-size:0.7rem;color:var(--text-muted);">"Deposit Pool"</div>
                    </div>
                </div>

                // Deposit pool explanation
                <div style="background:var(--bg-tertiary);border:1px solid var(--border);border-radius:var(--radius);padding:0.75rem;margin-bottom:1rem;">
                    <div style="font-size:0.8rem;color:var(--text-secondary);">
                        {format!(
                            "{} attendees locked {} each. Held until check-in.",
                            DemoEvent::registered(),
                            DemoEvent::deposit(),
                        )}
                    </div>
                </div>

                // Mock recent registrations
                <div style="display:flex;flex-direction:column;gap:0.4rem;margin-bottom:1.25rem;">
                    {vec![
                        ("Alex C.", "5 min ago"),
                        ("Sarah K.", "12 min ago"),
                        ("Mike T.", "23 min ago"),
                    ].into_iter().map(|(name, time)| {
                        view! {
                            <div style="display:flex;align-items:center;justify-content:space-between;padding:0.45rem 0.65rem;background:var(--bg-tertiary);border:1px solid var(--border);border-radius:var(--radius-sm);">
                                <div style="display:flex;align-items:center;gap:0.5rem;">
                                    <div style="width:22px;height:22px;border-radius:50%;background:rgba(99,102,241,0.2);display:flex;align-items:center;justify-content:center;font-size:0.5rem;font-weight:600;color:#6366f1;">
                                        {name.chars().next().unwrap_or('?')}
                                    </div>
                                    <span style="font-size:0.8rem;color:#fff;font-weight:500;">{name}</span>
                                </div>
                                <div style="display:flex;align-items:center;gap:0.5rem;">
                                    <span style="font-size:0.7rem;color:var(--success);">{"\u{2713} Deposited"}</span>
                                    <span style="font-size:0.65rem;color:var(--text-muted);">{time}</span>
                                </div>
                            </div>
                        }
                    }).collect::<Vec<_>>()}
                </div>

                {next_button("Event Day: Open Dashboard \u{2192}", go_dashboard)}
            </div>
        </Show>

        // ── Step 2: Dashboard (loading → content) ──
        <Show when=move || step.get() == OrganizerStep::Dashboard && simulating.get()>
            {loading_card("Loading dashboard...", "Fetching live data")}
        </Show>

        <Show when=move || step.get() == OrganizerStep::Dashboard && !simulating.get()>
            <div class="card" style="padding:1.5rem;">
                <div style="display:flex;align-items:center;gap:0.5rem;margin-bottom:1rem;">
                    <span style="font-size:1.1rem;">{"\u{26a1}"}</span>
                    <h2 style="font-size:1rem;font-weight:700;color:#fff;">"Live Dashboard"</h2>
                </div>

                // Live stats
                <div style="display:grid;grid-template-columns:repeat(3,1fr);gap:0.5rem;margin-bottom:1rem;">
                    <div style="background:var(--bg-tertiary);border:1px solid var(--border);border-radius:var(--radius-sm);padding:0.65rem;text-align:center;">
                        <div style="font-weight:700;color:#fff;font-size:1rem;">{format!("{}", DemoEvent::registered())}</div>
                        <div style="font-size:0.65rem;color:var(--text-muted);">"Registered"</div>
                    </div>
                    <div style="background:var(--bg-tertiary);border:1px solid var(--border);border-radius:var(--radius-sm);padding:0.65rem;text-align:center;">
                        <div style="font-weight:700;color:var(--success);font-size:1rem;">{format!("{}", DemoEvent::checked_in())}</div>
                        <div style="font-size:0.65rem;color:var(--text-muted);">"Checked In"</div>
                    </div>
                    <div style="background:var(--bg-tertiary);border:1px solid var(--border);border-radius:var(--radius-sm);padding:0.65rem;text-align:center;">
                        <div style="font-weight:700;color:var(--warning);font-size:1rem;">{format!("{}", DemoEvent::no_shows())}</div>
                        <div style="font-size:0.65rem;color:var(--text-muted);">"No-shows"</div>
                    </div>
                </div>

                // Progress bar
                <div style="margin-bottom:1rem;">
                    <div style="display:flex;justify-content:space-between;margin-bottom:0.25rem;">
                        <span style="font-size:0.75rem;color:var(--text-muted);">"Check-in Progress"</span>
                        <span style="font-size:0.75rem;color:var(--accent);">"95%"</span>
                    </div>
                    <div style="height:6px;background:var(--bg-tertiary);border-radius:9999px;overflow:hidden;">
                        <div style="width:95%;height:100%;background:linear-gradient(90deg,#6366f1,#a78bfa);border-radius:9999px;"></div>
                    </div>
                </div>

                // Live feed
                <div style="display:flex;flex-direction:column;gap:0.4rem;margin-bottom:1.25rem;">
                    {vec![
                        ("Alex C.", "\u{2713} Checked in", "2 min ago", true),
                        ("Sarah K.", "\u{2713} Checked in", "5 min ago", true),
                        ("Mike T.", "\u{2713} Checked in", "8 min ago", true),
                        ("Jamie L.", "\u{2717} No show", "Started", false),
                    ].into_iter().map(|(name, status, time, is_success)| {
                        view! {
                            <div style="display:flex;align-items:center;justify-content:space-between;padding:0.45rem 0.65rem;background:var(--bg-tertiary);border:1px solid var(--border);border-radius:var(--radius-sm);">
                                <div style="display:flex;align-items:center;gap:0.5rem;">
                                    <div style=move || format!(
                                        "width:22px;height:22px;border-radius:50%;background:{};display:flex;align-items:center;justify-content:center;font-size:0.5rem;font-weight:600;color:#fff;",
                                        if is_success { "rgba(34,197,94,0.3)" } else { "rgba(245,158,11,0.3)" },
                                    )>
                                        {name.chars().next().unwrap_or('?')}
                                    </div>
                                    <span style="font-size:0.8rem;color:#fff;font-weight:500;">{name}</span>
                                </div>
                                <div style="display:flex;align-items:center;gap:0.5rem;">
                                    <span style=move || format!("font-size:0.7rem;color:{};", if is_success { "var(--success)" } else { "var(--warning)" })>
                                        {status}
                                    </span>
                                    <span style="font-size:0.65rem;color:var(--text-muted);">{time}</span>
                                </div>
                            </div>
                        }
                    }).collect::<Vec<_>>()}
                </div>

                {next_button("See Final Payout \u{2192}", go_payout)}
            </div>
        </Show>

        // ── Step 3: Payout (final, instant — custom view) ──
        <Show when=move || step.get() == OrganizerStep::Payout>
            <div class="card" style="text-align:center;padding:2rem;">
                // Success checkmark
                <div style="width:64px;height:64px;border-radius:50%;background:rgba(99,102,241,0.15);display:inline-flex;align-items:center;justify-content:center;margin-bottom:1rem;">
                    <svg width="28" height="28" viewBox="0 0 24 24" fill="none" stroke="#6366f1" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round">
                        <polyline points="20 6 9 17 4 12"></polyline>
                    </svg>
                </div>

                <h2 style="font-size:1.2rem;font-weight:700;color:#fff;margin-bottom:1.25rem;">
                    "Event Complete!"
                </h2>

                // 2-column payout grid
                <div style="display:grid;grid-template-columns:1fr 1fr;gap:0.75rem;margin-bottom:1rem;">
                    <div style="background:rgba(34,197,94,0.08);border:1px solid rgba(34,197,94,0.2);border-radius:var(--radius);padding:1rem;text-align:center;">
                        <div style="font-size:0.7rem;color:var(--text-muted);margin-bottom:0.25rem;">"Refunded"</div>
                        <div style="font-weight:700;color:var(--success);font-size:0.95rem;">"142 attendees"</div>
                        <div style="font-size:0.7rem;color:var(--text-muted);margin-top:0.25rem;">"1.42 SOL + $1,846"</div>
                    </div>
                    <div style="background:rgba(99,102,241,0.08);border:1px solid rgba(99,102,241,0.2);border-radius:var(--radius);padding:1rem;text-align:center;">
                        <div style="font-size:0.7rem;color:var(--text-muted);margin-bottom:0.25rem;">"You Received"</div>
                        <div style="font-weight:700;color:#6366f1;font-size:0.95rem;">"8 no-shows"</div>
                        <div style="font-size:0.7rem;color:var(--text-muted);margin-top:0.25rem;">"0.08 SOL + $104"</div>
                    </div>
                </div>

                // NFT badges line
                <div style="background:var(--bg-tertiary);border:1px solid var(--border);border-radius:var(--radius);padding:0.65rem 0.85rem;margin-bottom:1.5rem;">
                    <div style="font-size:0.8rem;color:var(--text-secondary);">
                        "142 NFT badges minted (cNFT on Solana)"
                    </div>
                </div>

                // CTAs
                <div style="display:flex;flex-direction:column;gap:0.75rem;">
                    <A href="/login" attr:class="btn btn-primary" attr:style="width:100%;padding:0.85rem;font-size:0.95rem;">
                        "Start Your Own Event"
                    </A>
                    <A href="/" attr:class="btn btn-outline" attr:style="width:100%;">
                        "Back to Home"
                    </A>
                </div>
            </div>
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
        StaffStep::Done => 2,
    };

    let go_confirmed = move || {
        set_simulating.set(true);
        set_step.set(StaffStep::Confirmed);
        leptos::task::spawn_local(async move {
            gloo::timers::future::TimeoutFuture::new(1500).await;
            set_simulating.set(false);
        });
    };

    let go_done = move || {
        set_step.set(StaffStep::Done);
    };

    view! {
        {persona_header(role)}

        // Event context
        <div style="background:var(--bg-secondary);border:1px solid var(--border);border-radius:var(--radius);padding:0.65rem 1rem;margin-bottom:1.25rem;display:flex;align-items:center;gap:0.75rem;">
            <span style="font-size:1.1rem;">{"\u{1f3aa}"}</span>
            <div>
                <div style="font-weight:600;color:#fff;font-size:0.85rem;">{DemoEvent::name()}</div>
                <div style="font-size:0.75rem;color:var(--text-muted);">"Station: Door A"</div>
            </div>
        </div>

        {move || step_indicator(step_index(), 3, DemoRole::Staff.accent_color())}

        // ── Step 0: Scan ──
        <Show when=move || step.get() == StaffStep::Scan && !simulating.get()>
            <div class="card" style="text-align:center;padding:1.5rem;">
                <div style="display:flex;align-items:center;gap:0.5rem;margin-bottom:1rem;justify-content:center;">
                    <span style="font-size:1.1rem;">{"\u{1f4f7}"}</span>
                    <h2 style="font-size:1rem;font-weight:700;color:#fff;">"Scan QR Code"</h2>
                </div>

                // Mock scanner view
                <div style="width:180px;height:180px;margin:0 auto 1rem;border:2px solid #f59e0b;border-radius:12px;overflow:hidden;display:flex;align-items:center;justify-content:center;background:var(--bg-tertiary);position:relative;">
                    <div style="position:absolute;top:0;left:0;right:0;height:2px;background:linear-gradient(90deg,transparent,#f59e0b,transparent);animation:scan-line 1.5s ease-in-out infinite;"></div>
                    <div style="color:var(--text-muted);font-size:0.8rem;">"Point at QR..."</div>
                </div>

                // Simulated attendee
                <div style="background:var(--bg-tertiary);border:1px solid var(--border);border-radius:var(--radius);padding:0.65rem 0.85rem;margin-bottom:1.25rem;display:flex;align-items:center;gap:0.75rem;">
                    <div style="width:28px;height:28px;border-radius:50%;background:rgba(34,197,94,0.2);display:flex;align-items:center;justify-content:center;font-size:0.6rem;font-weight:600;color:var(--success);">
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

        // ── Step 0→1: Loading ──
        <Show when=move || step.get() == StaffStep::Scan && simulating.get()>
            {loading_card("Scanning...", "Verifying attendee")}
        </Show>

        // ── Step 1: Confirmed ──
        <Show when=move || step.get() == StaffStep::Confirmed && !simulating.get()>
            <div class="card" style="padding:1.5rem;">
                // Success flash
                <div style="text-align:center;margin-bottom:1rem;">
                    <div style="width:48px;height:48px;border-radius:50%;background:rgba(34,197,94,0.15);display:inline-flex;align-items:center;justify-content:center;border:1px solid rgba(34,197,94,0.3);">
                        <svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="var(--success)" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round">
                            <polyline points="20 6 9 17 4 12"></polyline>
                        </svg>
                    </div>
                </div>

                <h2 style="font-size:1rem;font-weight:700;color:#fff;text-align:center;margin-bottom:1rem;">"Checked In!"</h2>

                // Attendee detail
                <div style="background:var(--bg-tertiary);border:1px solid var(--border);border-radius:var(--radius);padding:0.85rem;margin-bottom:1.25rem;">
                    <div style="display:flex;align-items:center;gap:0.75rem;margin-bottom:0.5rem;">
                        <div style="width:32px;height:32px;border-radius:50%;background:rgba(34,197,94,0.2);display:flex;align-items:center;justify-content:center;font-size:0.65rem;font-weight:600;color:var(--success);">
                            "AC"
                        </div>
                        <div>
                            <div style="font-weight:600;color:#fff;font-size:0.85rem;">"Alex Chen"</div>
                            <div style="font-size:0.7rem;color:var(--text-muted);">"alex@dev.email"</div>
                        </div>
                    </div>
                    <div style="display:flex;gap:0.5rem;">
                        <span style="font-size:0.7rem;padding:0.15rem 0.5rem;background:rgba(34,197,94,0.15);color:var(--success);border-radius:9999px;border:1px solid rgba(34,197,94,0.3);">{"\u{2713} Checked in"}</span>
                        <span style="font-size:0.7rem;padding:0.15rem 0.5rem;background:var(--bg-secondary);color:var(--text-muted);border-radius:9999px;border:1px solid var(--border);">"General Admission"</span>
                    </div>
                </div>

                {next_button("See Summary \u{2192}", go_done)}
            </div>
        </Show>

        // ── Step 2: Done ──
        <Show when=move || step.get() == StaffStep::Done>
            <div class="card" style="text-align:center;padding:2rem;">
                // Success icon
                <div style="width:64px;height:64px;border-radius:50%;background:rgba(245,158,11,0.15);display:inline-flex;align-items:center;justify-content:center;margin-bottom:1rem;">
                    <svg width="28" height="28" viewBox="0 0 24 24" fill="none" stroke="#f59e0b" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round">
                        <polyline points="20 6 9 17 4 12"></polyline>
                    </svg>
                </div>

                <h2 style="font-size:1.2rem;font-weight:700;color:#fff;margin-bottom:1rem;">
                    "Staff journey complete!"
                </h2>

                // Highlight stat
                <div style="background:rgba(245,158,11,0.15);border:1px solid rgba(245,158,11,0.3);border-radius:var(--radius);padding:1rem;margin-bottom:1.5rem;">
                    <div style="font-size:0.8rem;color:var(--text-secondary);">"Avg. check-in time"</div>
                    <div style="font-weight:700;font-size:1.1rem;color:#f59e0b;">"< 2 seconds per person"</div>
                </div>

                <div style="font-size:0.8rem;color:var(--text-secondary);margin-bottom:1.5rem;">
                    "Lost QR? Search by name."
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
            </div>
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
        AttendeeStep::Register => 0,
        AttendeeStep::AtVenue => 1,
        AttendeeStep::Confirmed => 2,
        AttendeeStep::Quiz => 3,
        AttendeeStep::Claimed => 4,
    };

    let go_at_venue = move || {
        set_step.set(AttendeeStep::AtVenue);
    };

    let go_confirmed = move || {
        set_simulating.set(true);
        set_step.set(AttendeeStep::Confirmed);
        leptos::task::spawn_local(async move {
            gloo::timers::future::TimeoutFuture::new(1500).await;
            set_simulating.set(false);
        });
    };

    let go_quiz = move || {
        set_step.set(AttendeeStep::Quiz);
    };

    let go_claimed = move || {
        set_simulating.set(true);
        leptos::task::spawn_local(async move {
            gloo::timers::future::TimeoutFuture::new(2000).await;
            set_simulating.set(false);
            set_step.set(AttendeeStep::Claimed);
        });
    };

    let submit_quiz = move || {
        if quiz_answer.get().is_some() {
            set_quiz_submitted.set(true);
        }
    };

    view! {
        {persona_header(role)}

        // Event + deposit context
        <div style="background:var(--bg-secondary);border:1px solid var(--border);border-radius:var(--radius);padding:0.65rem 1rem;margin-bottom:1.25rem;display:flex;align-items:center;justify-content:space-between;">
            <div style="display:flex;align-items:center;gap:0.75rem;">
                <span style="font-size:1.1rem;">{"\u{1f3aa}"}</span>
                <div>
                    <div style="font-weight:600;color:#fff;font-size:0.85rem;">{DemoEvent::name()}</div>
                    <div style="font-size:0.75rem;color:var(--text-muted);">{DemoEvent::date()} {" \u{b7} "} {DemoEvent::location()}</div>
                </div>
            </div>
        </div>

        {move || step_indicator(step_index(), 5, DemoRole::Attendee.accent_color())}

        // ── Step 0: Register ──
        <Show when=move || step.get() == AttendeeStep::Register>
            <div class="card" style="padding:1.5rem;">
                <div style="display:flex;align-items:center;gap:0.5rem;margin-bottom:1rem;">
                    <span style="font-size:1.1rem;">{"\u{1f4dd}"}</span>
                    <h2 style="font-size:1rem;font-weight:700;color:#fff;">"Register"</h2>
                </div>

                // Event info card with deposit
                <div style="background:var(--bg-tertiary);border:1px solid var(--border);border-radius:var(--radius);padding:1rem;margin-bottom:1rem;">
                    <div style="font-weight:600;color:#fff;font-size:0.9rem;margin-bottom:0.25rem;">{DemoEvent::name()}</div>
                    <div style="font-size:0.8rem;color:var(--text-secondary);margin-bottom:0.75rem;">
                        {DemoEvent::date()} {" \u{b7} "} {DemoEvent::location()}
                    </div>
                    <div style="display:flex;align-items:center;justify-content:space-between;">
                        <div>
                            <div style="font-size:0.7rem;color:var(--text-muted);">"Deposit required"</div>
                            <div style="font-size:0.95rem;font-weight:700;color:#22c55e;">{DemoEvent::deposit()}</div>
                        </div>
                        <div style="font-size:0.7rem;color:var(--text-muted);">"per person"</div>
                    </div>
                </div>

                // Mock wallet indicator
                <div style="display:inline-flex;align-items:center;gap:0.5rem;background:rgba(34,197,94,0.1);border:1px solid rgba(34,197,94,0.3);border-radius:9999px;padding:0.35rem 0.85rem;margin-bottom:1rem;">
                    <span style="font-size:0.7rem;color:var(--success);">{"\u{25cf} Phantom"}</span>
                    <span style="font-size:0.7rem;color:var(--text-secondary);">{"\u{2014} 7xK9...f3Pz"}</span>
                </div>

                // Deposit summary
                <div style="background:rgba(34,197,94,0.08);border:1px solid rgba(34,197,94,0.2);border-radius:var(--radius);padding:0.75rem;margin-bottom:0.5rem;">
                    <div style="font-size:0.85rem;color:#fff;font-weight:600;">
                        {"Lock "} {DemoEvent::deposit()}
                    </div>
                </div>
                <div style="font-size:0.75rem;color:var(--text-muted);margin-bottom:1.25rem;">
                    "Held until check-in. Show up to get it back."
                </div>

                {next_button("Confirm Deposit", go_at_venue)}
            </div>
        </Show>

        // ── Step 1: AtVenue ──
        <Show when=move || step.get() == AttendeeStep::AtVenue && !simulating.get()>
            <div class="card" style="text-align:center;padding:1.5rem;">
                <div style="width:48px;height:48px;border-radius:10px;background:rgba(34,197,94,0.15);border:1px solid rgba(34,197,94,0.3);display:inline-flex;align-items:center;justify-content:center;font-size:1.25rem;margin-bottom:0.75rem;">{"\u{1f3ab}"}</div>
                <h2 style="font-size:1rem;font-weight:700;color:#fff;margin-bottom:1rem;">
                    "Show Your QR"
                </h2>

                // Mock QR code
                <div style="width:150px;height:150px;margin:0 auto 1rem;background:#fff;border-radius:8px;display:flex;align-items:center;justify-content:center;border:2px solid rgba(255,255,255,0.1);">
                    <div style="color:#000;font-size:0.65rem;font-weight:600;text-align:center;padding:0.5rem;">
                        "[ QR CODE ]\nalex@dev.email\nSolana Bangkok 2025"
                    </div>
                </div>

                {next_button("Staff Scans Your QR \u{2192}", go_confirmed)}
            </div>
        </Show>

        // ── Step 1→2: Loading ──
        <Show when=move || step.get() == AttendeeStep::AtVenue && simulating.get()>
            {loading_card("Scanning...", "Verifying your ticket")}
        </Show>

        // ── Step 2: Confirmed ──
        <Show when=move || step.get() == AttendeeStep::Confirmed && !simulating.get()>
            <div class="card" style="text-align:center;padding:1.5rem;">
                <div style="width:48px;height:48px;border-radius:50%;background:rgba(34,197,94,0.15);display:inline-flex;align-items:center;justify-content:center;border:1px solid rgba(34,197,94,0.3);margin-bottom:0.75rem;">
                    <svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="var(--success)" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round">
                        <polyline points="20 6 9 17 4 12"></polyline>
                    </svg>
                </div>
                <h2 style="font-size:1rem;font-weight:700;color:#fff;margin-bottom:0.5rem;">
                    "Checked In!"
                </h2>
                <div style="background:rgba(34,197,94,0.08);border:1px solid rgba(34,197,94,0.2);border-radius:var(--radius);padding:0.65rem;margin-bottom:1.25rem;">
                    <div style="font-size:0.8rem;color:var(--success);font-weight:500;">
                        {"Alex Chen \u{2014} checked in at 6:32 PM"}
                    </div>
                </div>

                {next_button("Take the Quiz \u{2192}", go_quiz)}
            </div>
        </Show>

        // ── Step 3: Quiz (combined with quiz-passed) ──
        <Show when=move || step.get() == AttendeeStep::Quiz && !quiz_submitted.get()>
            <div class="card" style="padding:1.5rem;">
                <div style="display:flex;align-items:center;gap:0.5rem;margin-bottom:1rem;">
                    <span style="font-size:1.1rem;">{"\u{1f9e0}"}</span>
                    <h2 style="font-size:1rem;font-weight:700;color:#fff;">"Quick Quiz"</h2>
                </div>

                <div style="margin-bottom:1rem;">
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

        // ── Step 3b: Quiz submitted → claiming ──
        <Show when=move || step.get() == AttendeeStep::Quiz && quiz_submitted.get() && simulating.get()>
            {loading_card("Minting NFT badge...", "Compressing NFT on Solana")}
        </Show>

        <Show when=move || step.get() == AttendeeStep::Quiz && quiz_submitted.get() && !simulating.get()>
            <div class="card" style="text-align:center;padding:1.5rem;">
                <div style="width:48px;height:48px;border-radius:50%;background:rgba(34,197,94,0.15);display:inline-flex;align-items:center;justify-content:center;border:1px solid rgba(34,197,94,0.3);margin-bottom:0.75rem;">
                    <svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="var(--success)" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round">
                        <polyline points="20 6 9 17 4 12"></polyline>
                    </svg>
                </div>
                <h2 style="font-size:1rem;font-weight:700;color:#fff;margin-bottom:0.5rem;">
                    "Quiz Passed!"
                </h2>
                <div style="font-size:0.8rem;color:var(--text-secondary);margin-bottom:1rem;">
                    "Claim your NFT badge and deposit refund."
                </div>
                {next_button("Claim NFT + Refund \u{2192}", go_claimed)}
            </div>
        </Show>

        // ── Step 4: Claimed (final — custom view) ──
        <Show when=move || step.get() == AttendeeStep::Claimed>
            <div class="card" style="text-align:center;padding:2rem;">
                // Success icon (green gradient)
                <div style="width:64px;height:64px;border-radius:50%;background:linear-gradient(135deg,#22c55e,#16a34a);display:inline-flex;align-items:center;justify-content:center;margin-bottom:1rem;">
                    <svg width="32" height="32" viewBox="0 0 24 24" fill="none" stroke="#fff" stroke-width="3" stroke-linecap="round" stroke-linejoin="round">
                        <polyline points="20 6 9 17 4 12"></polyline>
                    </svg>
                </div>

                <h2 style="font-size:1.25rem;font-weight:700;color:#fff;margin-bottom:1rem;">
                    "NFT Badge Claimed!"
                </h2>

                // Mock NFT card
                <div style="background:var(--bg-tertiary);border:1px solid var(--border);border-radius:var(--radius);overflow:hidden;margin-bottom:1rem;text-align:left;">
                    <div style="height:100px;background:linear-gradient(135deg,#22c55e,#16a34a,#4ade80);display:flex;align-items:center;justify-content:center;">
                        <span style="font-size:2rem;">{"\u{1f3ab}"}</span>
                    </div>
                    <div style="padding:0.85rem;">
                        <div style="font-weight:600;color:#fff;font-size:0.85rem;margin-bottom:0.2rem;">
                            "BeThere Proof of Attendance"
                        </div>
                        <div style="font-size:0.75rem;color:var(--text-secondary);margin-bottom:0.5rem;">
                            {DemoEvent::name()}
                        </div>
                        <div style="display:flex;gap:0.5rem;">
                            <span style="font-size:0.65rem;padding:0.15rem 0.45rem;background:rgba(34,197,94,0.15);color:var(--success);border-radius:9999px;border:1px solid rgba(34,197,94,0.3);">
                                "cNFT"
                            </span>
                            <span style="font-size:0.65rem;padding:0.15rem 0.45rem;background:rgba(99,102,241,0.15);color:#6366f1;border-radius:9999px;border:1px solid rgba(99,102,241,0.3);">
                                "Solana"
                            </span>
                        </div>
                    </div>
                </div>

                // Deposit refund
                <div style="background:rgba(34,197,94,0.08);border:1px solid rgba(34,197,94,0.2);border-radius:var(--radius);padding:0.75rem;margin-bottom:1.5rem;">
                    <div style="font-size:0.75rem;color:var(--text-secondary);">"Deposit Refunded"</div>
                    <div style="font-weight:700;color:var(--success);font-size:1.05rem;">{DemoEvent::deposit()}</div>
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
            </div>
        </Show>
    }
}
