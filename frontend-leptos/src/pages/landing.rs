//! Landing page — public marketing page for BeThere.
//!
//! Showcases the platform with hero, problem/solution, how-it-works steps,
//! organizer and attendee pitches, and footer branding.
//! No backend calls — purely static marketing content with SPA navigation.

use leptos::prelude::*;
use leptos_router::components::A;

/// Waitlist signup form component.
#[component]
fn WaitlistForm() -> impl IntoView {
    let (email, set_email) = signal(String::new());
    let (submitted, set_submitted) = signal(false);
    let (error, set_error) = signal(None::<String>);
    let (submitting, set_submitting) = signal(false);
    let (already_registered, set_already_registered) = signal(false);

    let handle_submit = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        let email_val = email.get().trim().to_string();

        if email_val.is_empty() || !email_val.contains('@') || !email_val.contains('.') {
            set_error.set(Some("Please enter a valid email".to_string()));
            return;
        }

        set_error.set(None);
        set_submitting.set(true);

        leptos::task::spawn_local(async move {
            let window = web_sys::window().expect("no window");
            let origin = window.location().origin().unwrap_or("http://localhost:8787".to_string());
            let url = format!("{origin}/api/waitlist");

            let body = serde_json::json!({ "email": email_val });

            let request = match gloo::net::http::Request::post(&url)
                .header("Content-Type", "application/json")
                .body(serde_json::to_string(&body).unwrap_or_default())
            {
                Ok(req) => req,
                Err(e) => {
                    set_error.set(Some(format!("Failed to submit: {e}")));
                    set_submitting.set(false);
                    return;
                }
            };

            match request.send().await {
                Ok(response) => {
                    if response.ok() {
                        // Parse JSON to check for duplicate
                        match response.json::<serde_json::Value>().await {
                            Ok(body) => {
                                if body.get("code").and_then(|v| v.as_str()) == Some("duplicate") {
                                    set_already_registered.set(true);
                                } else if body.get("success").and_then(|v| v.as_bool()) == Some(true) {
                                    set_submitted.set(true);
                                } else {
                                    let msg = body.get("error").and_then(|v| v.as_str()).unwrap_or("Something went wrong");
                                    set_error.set(Some(msg.to_string()));
                                }
                            }
                            Err(_) => {
                                // Can't parse JSON — treat as success
                                set_submitted.set(true);
                            }
                        }
                    } else {
                        set_error.set(Some("Something went wrong. Please try again.".to_string()));
                    }
                }
                Err(e) => {
                    set_error.set(Some(format!("Network error: {e}")));
                }
            }
            set_submitting.set(false);
        });
    };

    view! {
        <Show
            when=move || submitted.get() || already_registered.get()
            fallback=|| view! { <div></div> }
        >
            <div style="padding:1.5rem;background:var(--success-bg);border:1px solid var(--success-border);border-radius:var(--radius);text-align:center;">
                <div style="font-size:1.25rem;margin-bottom:0.5rem;">"✓"</div>
                <div style="font-weight:600;color:var(--success);margin-bottom:0.25rem;">
                    {move || if already_registered.get() { "You're already on the list!" } else { "You're on the list!" }}
                </div>
                <div style="font-size:0.85rem;color:var(--text-secondary);">"We'll reach out when we're ready to onboard new events."</div>
            </div>
        </Show>
        <Show
            when=move || !submitted.get() && !already_registered.get()
            fallback=|| view! { <div></div> }
        >
            <form on:submit=handle_submit style="display:flex;gap:0.5rem;max-width:400px;margin:0 auto;">
                <input
                    type="email"
                    placeholder="your@email.com"
                    prop:value=move || email.get()
                    on:input=move |ev| set_email.set(event_target_value(&ev))
                    disabled=move || submitting.get()
                    style="flex:1;min-width:0;padding:0.75rem 1rem;border-radius:var(--radius-sm);border:1px solid var(--border);background:var(--bg-secondary);color:var(--text-primary);font-size:0.9rem;outline:none;"
                />
                <button
                    type="submit"
                    disabled=move || submitting.get() || email.get().trim().is_empty()
                    class="btn btn-primary"
                    style="white-space:nowrap;padding:0.75rem 1.25rem;"
                >
                    {move || if submitting.get() { "Joining..." } else { "Join Waitlist" }}
                </button>
            </form>
            <Show
                when=move || error.get().is_some()
                fallback=|| view! { <div></div> }
            >
                <p style="color:var(--danger);font-size:0.8rem;margin-top:0.5rem;">
                    {move || error.get().unwrap_or_default()}
                </p>
            </Show>
        </Show>
    }
}

// ── Swimlane Types ──────────────────────────────────────────────────────────

#[derive(Clone, Copy, PartialEq, Eq)]
enum SwimlaneRole {
    Organizer,
    Staff,
    Attendee,
}

impl SwimlaneRole {
    fn label(self) -> &'static str {
        match self {
            Self::Organizer => "Organizer",
            Self::Staff => "Staff",
            Self::Attendee => "Attendee",
        }
    }

    fn emoji(self) -> &'static str {
        match self {
            Self::Organizer => "\u{1f3af}",
            Self::Staff => "\u{1f4f1}",
            Self::Attendee => "\u{1f3ab}",
        }
    }

    fn accent(self) -> &'static str {
        match self {
            Self::Organizer => "#6366f1",
            Self::Staff => "#f59e0b",
            Self::Attendee => "#22c55e",
        }
    }

    fn accent_bg(self) -> &'static str {
        match self {
            Self::Organizer => "rgba(99,102,241,0.12)",
            Self::Staff => "rgba(245,158,11,0.12)",
            Self::Attendee => "rgba(34,197,94,0.12)",
        }
    }

    fn accent_border(self) -> &'static str {
        match self {
            Self::Organizer => "rgba(99,102,241,0.35)",
            Self::Staff => "rgba(245,158,11,0.35)",
            Self::Attendee => "rgba(34,197,94,0.35)",
        }
    }

    fn steps(self) -> &'static [SwimlaneStep] {
        static ORG: &[SwimlaneStep] = &[
            SwimlaneStep { icon: "\u{1f4cb}", title: "Create Event", desc: "Set name, capacity, and deposit" },
            SwimlaneStep { icon: "\u{1f4b0}", title: "150 Registered", desc: "Deposits pool to 1.5 SOL + $1,950" },
            SwimlaneStep { icon: "\u{1f4ca}", title: "Live Dashboard", desc: "Track check-ins & no-shows" },
            SwimlaneStep { icon: "\u{1f4b8}", title: "Auto Payout", desc: "Refund attendees, keep no-shows" },
        ];
        static STAFF: &[SwimlaneStep] = &[
            SwimlaneStep { icon: "\u{1f4f7}", title: "Open Scanner", desc: "Point camera at attendee QR" },
            SwimlaneStep { icon: "\u{2705}", title: "Instant Confirm", desc: "Verified in < 2 seconds" },
            SwimlaneStep { icon: "\u{1f389}", title: "Session Done", desc: "142 checked in, all smooth" },
        ];
        static ATT: &[SwimlaneStep] = &[
            SwimlaneStep { icon: "\u{1f3ab}", title: "Register & Deposit", desc: "Lock 0.01 SOL + $13 USDC" },
            SwimlaneStep { icon: "\u{1f4f1}", title: "Show QR Code", desc: "At venue, display check-in code" },
            SwimlaneStep { icon: "\u{2705}", title: "Get Scanned", desc: "Staff scans — instant confirm" },
            SwimlaneStep { icon: "\u{1f9e0}", title: "Quick Quiz", desc: "Prove you paid attention" },
            SwimlaneStep { icon: "\u{1f4b0}", title: "Claim Refund + Badge", desc: "Deposit back + cNFT forever" },
        ];
        match self {
            Self::Organizer => ORG,
            Self::Staff => STAFF,
            Self::Attendee => ATT,
        }
    }
}

struct SwimlaneStep {
    icon: &'static str,
    title: &'static str,
    #[allow(dead_code)]
    desc: &'static str,
}

/// Render the mockup card for a given role + step index.
fn swimlane_mockup(role: SwimlaneRole, step: usize) -> impl IntoView {
    match role {
        SwimlaneRole::Organizer => match step {
            // Create Event — form card
            0 => view! {
                <div style="background:var(--bg-secondary);border:1px solid var(--border);border-radius:var(--radius);padding:1.25rem;font-size:0.8rem;">
                    <div style="font-weight:600;color:#fff;margin-bottom:0.75rem;">"New Event"</div>
                    <div style="display:flex;flex-direction:column;gap:0.5rem;">
                        <div style="background:rgba(255,255,255,0.04);border:1px solid var(--border);border-radius:6px;padding:0.5rem 0.65rem;color:var(--text-secondary);">"Solana Bangkok Meetup 2025"</div>
                        <div style="display:flex;gap:0.5rem;">
                            <div style="flex:1;background:rgba(255,255,255,0.04);border:1px solid var(--border);border-radius:6px;padding:0.5rem 0.65rem;color:var(--text-secondary);">"\u{1f4cd} Bangkok"</div>
                            <div style="flex:1;background:rgba(255,255,255,0.04);border:1px solid var(--border);border-radius:6px;padding:0.5rem 0.65rem;color:var(--text-secondary);">"Cap: 200"</div>
                        </div>
                        <div style="background:rgba(99,102,241,0.08);border:1px solid rgba(99,102,241,0.25);border-radius:6px;padding:0.5rem 0.65rem;color:#818cf8;font-weight:500;">"Deposit: 0.01 SOL + $13 USDC"</div>
                    </div>
                    <div style="margin-top:0.75rem;background:#6366f1;color:#fff;border-radius:6px;padding:0.5rem;text-align:center;font-weight:600;">"Create Event"</div>
                </div>
            }.into_any(),
            // Registrations — deposit pool
            1 => view! {
                <div style="background:var(--bg-secondary);border:1px solid var(--border);border-radius:var(--radius);padding:1.25rem;font-size:0.8rem;">
                    <div style="display:flex;align-items:center;gap:0.5rem;margin-bottom:0.75rem;">
                        <span style="font-size:1rem;">"\u{1f4b0}"</span>
                        <span style="font-weight:600;color:#fff;">"Deposit Pool"</span>
                    </div>
                    <div style="display:flex;gap:1rem;margin-bottom:0.75rem;">
                        <div>
                            <div style="font-size:1.1rem;font-weight:700;color:#818cf8;">"1.5 SOL"</div>
                            <div style="color:var(--text-secondary);font-size:0.7rem;">"+ $1,950 USDC"</div>
                        </div>
                        <div style="flex:1;"></div>
                        <div style="text-align:right;">
                            <div style="font-size:1.1rem;font-weight:700;color:#fff;">"150"</div>
                            <div style="color:var(--text-secondary);font-size:0.7rem;">"attendees"</div>
                        </div>
                    </div>
                    <div style="background:rgba(99,102,241,0.1);border-radius:9999px;height:6px;overflow:hidden;">
                        <div style="width:75%;height:100%;background:linear-gradient(90deg,#6366f1,#818cf8);border-radius:9999px;"></div>
                    </div>
                    <div style="color:var(--text-secondary);font-size:0.7rem;margin-top:0.35rem;text-align:right;">"75% of capacity"</div>
                </div>
            }.into_any(),
            // Dashboard — live stats
            2 => view! {
                <div style="background:var(--bg-secondary);border:1px solid var(--border);border-radius:var(--radius);padding:1.25rem;font-size:0.8rem;">
                    <div style="font-weight:600;color:#fff;margin-bottom:0.75rem;">"\u{1f4ca} Live Dashboard"</div>
                    <div style="display:grid;grid-template-columns:1fr 1fr 1fr;gap:0.5rem;margin-bottom:0.75rem;">
                        <div style="text-align:center;background:rgba(99,102,241,0.08);border-radius:8px;padding:0.5rem;">
                            <div style="font-size:1.1rem;font-weight:700;color:#818cf8;">"150"</div>
                            <div style="color:var(--text-secondary);font-size:0.65rem;">"registered"</div>
                        </div>
                        <div style="text-align:center;background:rgba(34,197,94,0.08);border-radius:8px;padding:0.5rem;">
                            <div style="font-size:1.1rem;font-weight:700;color:#22c55e;">"142"</div>
                            <div style="color:var(--text-secondary);font-size:0.65rem;">"checked in"</div>
                        </div>
                        <div style="text-align:center;background:rgba(239,68,68,0.08);border-radius:8px;padding:0.5rem;">
                            <div style="font-size:1.1rem;font-weight:700;color:#ef4444;">"8"</div>
                            <div style="color:var(--text-secondary);font-size:0.65rem;">"no-show"</div>
                        </div>
                    </div>
                    <div style="background:rgba(34,197,94,0.1);border-radius:9999px;height:6px;overflow:hidden;">
                        <div style="width:95%;height:100%;background:linear-gradient(90deg,#22c55e,#4ade80);border-radius:9999px;"></div>
                    </div>
                    <div style="color:var(--text-secondary);font-size:0.7rem;margin-top:0.35rem;">"95% attendance"</div>
                </div>
            }.into_any(),
            // Payout — refund + received
            _ => view! {
                <div style="background:var(--bg-secondary);border:1px solid var(--border);border-radius:var(--radius);padding:1.25rem;font-size:0.8rem;">
                    <div style="font-weight:600;color:#fff;margin-bottom:0.75rem;">"\u{1f4b8} Payout Summary"</div>
                    <div style="display:grid;grid-template-columns:1fr 1fr;gap:0.75rem;">
                        <div style="background:rgba(34,197,94,0.08);border:1px solid rgba(34,197,94,0.2);border-radius:8px;padding:0.65rem;">
                            <div style="color:#22c55e;font-weight:600;font-size:0.7rem;margin-bottom:0.35rem;">"\u{2705} Refunded"</div>
                            <div style="color:#fff;font-weight:700;">"142"</div>
                            <div style="color:var(--text-secondary);font-size:0.7rem;">"1.42 SOL + $1,846"</div>
                        </div>
                        <div style="background:rgba(245,158,11,0.08);border:1px solid rgba(245,158,11,0.2);border-radius:8px;padding:0.65rem;">
                            <div style="color:#f59e0b;font-weight:600;font-size:0.7rem;margin-bottom:0.35rem;">"\u{1f4b0} You Received"</div>
                            <div style="color:#fff;font-weight:700;">"8"</div>
                            <div style="color:var(--text-secondary);font-size:0.7rem;">"0.08 SOL + $104"</div>
                        </div>
                    </div>
                </div>
            }.into_any(),
        },
        SwimlaneRole::Staff => match step {
            // Scan — camera frame
            0 => view! {
                <div style="background:var(--bg-secondary);border:1px solid var(--border);border-radius:var(--radius);padding:1.25rem;font-size:0.8rem;text-align:center;">
                    <div style="background:#0a0a0a;border:2px solid var(--border);border-radius:12px;padding:1.5rem 1rem;margin-bottom:0.75rem;position:relative;">
                        <div style="position:absolute;top:8px;left:8px;width:24px;height:24px;border-top:3px solid #f59e0b;border-left:3px solid #f59e0b;border-radius:4px 0 0 0;"></div>
                        <div style="position:absolute;top:8px;right:8px;width:24px;height:24px;border-top:3px solid #f59e0b;border-right:3px solid #f59e0b;border-radius:0 4px 0 0;"></div>
                        <div style="position:absolute;bottom:8px;left:8px;width:24px;height:24px;border-bottom:3px solid #f59e0b;border-left:3px solid #f59e0b;border-radius:0 0 0 4px;"></div>
                        <div style="position:absolute;bottom:8px;right:8px;width:24px;height:24px;border-bottom:3px solid #f59e0b;border-right:3px solid #f59e0b;border-radius:0 0 4px 0;"></div>
                        <div style="color:var(--text-secondary);font-size:0.75rem;">"\u{1f4f7} Point at attendee QR code"</div>
                    </div>
                    <div style="color:#f59e0b;font-size:0.75rem;font-weight:600;">"Scanning..."</div>
                </div>
            }.into_any(),
            // Confirmed — success card
            1 => view! {
                <div style="background:var(--bg-secondary);border:1px solid rgba(34,197,94,0.3);border-radius:var(--radius);padding:1.25rem;font-size:0.8rem;text-align:center;">
                    <div style="width:2.5rem;height:2.5rem;border-radius:50%;background:rgba(34,197,94,0.15);display:inline-flex;align-items:center;justify-content:center;font-size:1.25rem;margin-bottom:0.5rem;">"\u{2705}"</div>
                    <div style="font-weight:700;color:#22c55e;margin-bottom:0.25rem;">"Checked In!"</div>
                    <div style="color:#fff;font-weight:600;">"Alex Chen"</div>
                    <div style="color:var(--text-secondary);font-size:0.7rem;">"Solana Bangkok 2025"</div>
                    <div style="color:var(--text-secondary);font-size:0.7rem;margin-top:0.25rem;">"Jul 15 \u{00b7} 2:03 PM"</div>
                </div>
            }.into_any(),
            // Done — summary
            _ => view! {
                <div style="background:var(--bg-secondary);border:1px solid var(--border);border-radius:var(--radius);padding:1.25rem;font-size:0.8rem;text-align:center;">
                    <div style="font-size:1.25rem;margin-bottom:0.5rem;">"\u{1f389}"</div>
                    <div style="font-weight:700;color:#fff;margin-bottom:0.25rem;">"Session Complete"</div>
                    <div style="display:flex;justify-content:center;gap:1rem;margin-bottom:0.5rem;">
                        <div>
                            <div style="font-weight:700;color:#f59e0b;">"142"</div>
                            <div style="color:var(--text-secondary);font-size:0.65rem;">"checked in"</div>
                        </div>
                        <div>
                            <div style="font-weight:700;color:#fff;">"< 2s"</div>
                            <div style="color:var(--text-secondary);font-size:0.65rem;">"avg time"</div>
                        </div>
                    </div>
                    <div style="color:var(--text-secondary);font-size:0.7rem;">"Lost QR? Search by name \u{2192}"</div>
                </div>
            }.into_any(),
        },
        SwimlaneRole::Attendee => match step {
            // Register & Deposit
            0 => view! {
                <div style="background:var(--bg-secondary);border:1px solid var(--border);border-radius:var(--radius);padding:1.25rem;font-size:0.8rem;">
                    <div style="font-weight:600;color:#fff;margin-bottom:0.5rem;">"Solana Bangkok Meetup 2025"</div>
                    <div style="color:var(--text-secondary);font-size:0.7rem;margin-bottom:0.75rem;">"Jul 15 \u{00b7} Bangkok, Thailand"</div>
                    <div style="display:flex;align-items:center;gap:0.4rem;background:rgba(255,255,255,0.04);border:1px solid var(--border);border-radius:9999px;padding:0.3rem 0.65rem;margin-bottom:0.65rem;">
                        <span style="color:#ab9ff2;">"\u{25cf}"</span>
                        <span style="color:var(--text-secondary);font-size:0.7rem;">"Phantom \u{2014} 7xK9...f3Pz"</span>
                    </div>
                    <div style="background:rgba(34,197,94,0.08);border:1px solid rgba(34,197,94,0.2);border-radius:8px;padding:0.5rem 0.65rem;">
                        <div style="color:#22c55e;font-weight:600;font-size:0.7rem;">"Deposit to lock"</div>
                        <div style="color:#fff;font-weight:700;">"0.01 SOL + $13 USDC"</div>
                    </div>
                    <div style="margin-top:0.65rem;background:#22c55e;color:#fff;border-radius:6px;padding:0.5rem;text-align:center;font-weight:600;font-size:0.75rem;">"Confirm Deposit"</div>
                </div>
            }.into_any(),
            // Show QR
            1 => view! {
                <div style="background:var(--bg-secondary);border:1px solid var(--border);border-radius:var(--radius);padding:1.25rem;font-size:0.8rem;text-align:center;">
                    <div style="display:inline-block;background:#fff;border-radius:12px;padding:0.75rem;margin-bottom:0.75rem;">
                        <div style="width:80px;height:80px;display:grid;grid-template-columns:repeat(8,1fr);gap:1px;">
                            {(0..64).map(|i| view! { <div style=format!("width:8px;height:8px;border-radius:1px;background:{};", if i % 3 == 0 { "#000" } else if i % 5 == 0 { "#333" } else { "#fff" })></div> }).collect_view()}
                        </div>
                    </div>
                    <div style="color:#fff;font-weight:600;">"Alex Chen"</div>
                    <div style="color:var(--text-secondary);font-size:0.7rem;">"Solana Bangkok 2025"</div>
                </div>
            }.into_any(),
            // Get Scanned
            2 => view! {
                <div style="background:var(--bg-secondary);border:1px solid rgba(34,197,94,0.3);border-radius:var(--radius);padding:1.25rem;font-size:0.8rem;text-align:center;">
                    <div style="width:2.5rem;height:2.5rem;border-radius:50%;background:rgba(34,197,94,0.15);display:inline-flex;align-items:center;justify-content:center;font-size:1.25rem;margin-bottom:0.5rem;">"\u{2705}"</div>
                    <div style="font-weight:700;color:#22c55e;margin-bottom:0.25rem;">"Checked In!"</div>
                    <div style="color:var(--text-secondary);font-size:0.7rem;">"Jul 15, 2025 \u{00b7} 2:03 PM"</div>
                    <div style="color:var(--text-secondary);font-size:0.7rem;margin-top:0.15rem;">"Solana Bangkok Meetup"</div>
                </div>
            }.into_any(),
            // Quiz
            3 => view! {
                <div style="background:var(--bg-secondary);border:1px solid var(--border);border-radius:var(--radius);padding:1.25rem;font-size:0.8rem;">
                    <div style="font-weight:600;color:#fff;margin-bottom:0.5rem;">"\u{1f9e0} Event Quiz"</div>
                    <div style="color:var(--text-secondary);font-size:0.75rem;margin-bottom:0.65rem;">"What does BeThere use to prove attendance?"</div>
                    <div style="display:flex;flex-direction:column;gap:0.35rem;">
                        <div style="background:rgba(255,255,255,0.03);border:1px solid var(--border);border-radius:6px;padding:0.4rem 0.6rem;color:var(--text-secondary);font-size:0.7rem;">"\u{25cb} PDF certificate"</div>
                        <div style="background:rgba(34,197,94,0.08);border:1px solid rgba(34,197,94,0.3);border-radius:6px;padding:0.4rem 0.6rem;color:#22c55e;font-size:0.7rem;font-weight:600;">"\u{25cf} Compressed NFT badge"</div>
                        <div style="background:rgba(255,255,255,0.03);border:1px solid var(--border);border-radius:6px;padding:0.4rem 0.6rem;color:var(--text-secondary);font-size:0.7rem;">"\u{25cb} Email receipt"</div>
                        <div style="background:rgba(255,255,255,0.03);border:1px solid var(--border);border-radius:6px;padding:0.4rem 0.6rem;color:var(--text-secondary);font-size:0.7rem;">"\u{25cb} Paper ticket"</div>
                    </div>
                </div>
            }.into_any(),
            // Claim Refund + Badge
            _ => view! {
                <div style="background:var(--bg-secondary);border:1px solid var(--border);border-radius:var(--radius);padding:1.25rem;font-size:0.8rem;text-align:center;">
                    <div style="display:inline-block;background:linear-gradient(135deg,#22c55e,#4ade80);border-radius:12px;padding:0.65rem 1rem;margin-bottom:0.65rem;">
                        <div style="font-weight:700;color:#fff;font-size:0.9rem;">"\u{1f3ab} BeThere"</div>
                        <div style="color:rgba(255,255,255,0.8);font-size:0.6rem;">"cNFT \u{00b7} Solana"</div>
                    </div>
                    <div style="color:#fff;font-weight:600;margin-bottom:0.25rem;">"\u{1f4b0} Refund Claimed!"</div>
                    <div style="color:var(--text-secondary);font-size:0.7rem;">"0.01 SOL + $13 USDC returned"</div>
                    <div style="margin-top:0.65rem;background:#22c55e;color:#fff;border-radius:6px;padding:0.5rem;font-weight:600;font-size:0.75rem;">"Claim to Wallet"</div>
                </div>
            }.into_any(),
        },
    }
}

/// Landing page component.
#[component]
pub fn Landing() -> impl IntoView {
    let (active_role, set_active_role) = signal(SwimlaneRole::Attendee);
    let (active_step, set_active_step) = signal(0usize);

    view! {
        <div style="min-height:100vh;width:100%;">

            // ===== Nav Bar =====
            <nav style="position:sticky;top:0;z-index:100;background:rgba(15,15,15,0.85);backdrop-filter:blur(12px);border-bottom:1px solid var(--border);">
                <div style="max-width:960px;margin:0 auto;padding:0.85rem 1.5rem;display:flex;align-items:center;justify-content:space-between;">
                    <div style="display:flex;align-items:center;gap:0.5rem;">
                        <span style="font-size:1.25rem;font-weight:800;letter-spacing:0.06em;background:linear-gradient(135deg,#818cf8 0%,#6366f1 40%,#a78bfa 100%);-webkit-background-clip:text;-webkit-text-fill-color:transparent;background-clip:text;">
                            "BeThere"
                        </span>
                    </div>
                    <div class="landing-nav-links">
                        <a href="#features">"Features"</a>
                        <a href="#how-it-works">"How it works"</a>
                        <a href="#faq">"FAQ"</a>
                    </div>
                    <div style="display:flex;align-items:center;gap:0.75rem;">
                        <A href="/login" attr:class="btn btn-outline btn-sm">
                            "Sign In"
                        </A>
                    </div>
                </div>
            </nav>

            // ===== Hero =====
            <section style="max-width:960px;margin:0 auto;padding:5rem 1.5rem 4rem;text-align:center;">
                // Solana pill badge
                <div class="solana-pill">
                    <svg viewBox="0 0 397 311" xmlns="http://www.w3.org/2000/svg">
                        <path d="M64.6 237.9c2.4-2.4 5.7-3.8 9.2-3.8h317.4c5.8 0 8.7 7 4.6 11.1l-62.7 62.7c-2.4 2.4-5.7 3.8-9.2 3.8H6.5c-5.8 0-8.7-7-4.6-11.1l62.7-62.7z" fill="currentColor"/>
                        <path d="M64.6 3.8C67.1 1.4 70.4 0 73.8 0h317.4c5.8 0 8.7 7 4.6 11.1l-62.7 62.7c-2.4 2.4-5.7 3.8-9.2 3.8H6.5c-5.8 0-8.7-7-4.6-11.1L64.6 3.8z" fill="currentColor"/>
                        <path d="M333.1 120.1c-2.4-2.4-5.7-3.8-9.2-3.8H6.5c-5.8 0-8.7 7-4.6 11.1l62.7 62.7c2.4 2.4 5.7 3.8 9.2 3.8h317.4c5.8 0 8.7-7 4.6-11.1l-62.7-62.7z" fill="currentColor"/>
                    </svg>
                    "Powered by Solana"
                </div>

                <h1 style="font-size:clamp(1.75rem,5vw,2.75rem);font-weight:800;line-height:1.15;margin-bottom:1.25rem;color:#fff;">
                    "Commit. Show up."
                    <br />
                    <span style="background:linear-gradient(135deg,#818cf8,#6366f1,#a78bfa);-webkit-background-clip:text;-webkit-text-fill-color:transparent;background-clip:text;">
                        "Get your money back."
                    </span>
                </h1>
                <p style="font-size:1.1rem;color:var(--text-secondary);max-width:520px;margin:0 auto 2.25rem;line-height:1.6;">
                    "Put down a small deposit to reserve your spot. Show up, prove you paid attention with a quick quiz, and get every cent back — plus a digital badge you own forever. Don't show up? The organizer keeps your deposit. Simple."
                </p>
                <div style="display:flex;flex-wrap:wrap;gap:0.75rem;justify-content:center;">
                    <A href="/demo" attr:class="btn btn-primary" attr:style="padding:0.85rem 2rem;font-size:1rem;">
                        "Try Free Demo"
                    </A>
                    <A href="/login" attr:class="btn btn-outline" attr:style="padding:0.85rem 2rem;font-size:1rem;">
                        "Sign In"
                    </A>
                    <a href="#how-it-works" class="btn btn-outline" style="padding:0.85rem 2rem;font-size:1rem;">
                        "How It Works"
                    </a>
                </div>
            </section>

            // ===== Social Proof =====
            // Social proof — real users + CTA for organizers
            <section class="social-proof">
                <div class="social-proof-label">"Alpha · Building with"</div>
                <div class="social-proof-logos">
                    <a
                        href="https://github.com/solana-thailand"
                        target="_blank"
                        rel="noopener noreferrer"
                        class="social-proof-pill"
                        style="text-decoration:none;color:var(--text-secondary);border-color:rgba(99,102,241,0.3);"
                    >
                        "Solana Developer Thailand"
                    </a>
                    <a href="#waitlist" class="social-proof-pill" style="text-decoration:none;color:var(--accent);border-color:rgba(99,102,241,0.4);cursor:pointer;">
                        "Want to join? → Get in touch"
                    </a>
                </div>
            </section>

            // ===== Problem / Features =====
            <section id="features" style="max-width:960px;margin:0 auto;padding:3rem 1.5rem 4rem;">
                <div style="text-align:center;margin-bottom:2.5rem;">
                    <h2 style="font-size:1.5rem;font-weight:700;color:#fff;margin-bottom:0.5rem;">
                        "Events have a no-show problem"
                    </h2>
                    <p style="color:var(--text-secondary);font-size:0.95rem;">
                        "Up to 40% of registered attendees don't show up. Organizers pay for empty seats."
                    </p>
                </div>
                <div style="display:grid;grid-template-columns:repeat(auto-fit,minmax(240px,1fr));gap:1rem;">

                    <div class="card" style="text-align:center;padding:1.5rem;">
                        <div class="landing-svg-icon icon-clipboard">
                            <svg viewBox="0 0 24 24">
                                <path d="M16 4h2a2 2 0 0 1 2 2v14a2 2 0 0 1-2 2H6a2 2 0 0 1-2-2V6a2 2 0 0 1 2-2h2"/>
                                <rect x="8" y="2" width="8" height="4" rx="1" ry="1"/>
                            </svg>
                        </div>
                        <h3 style="font-size:0.95rem;font-weight:600;color:#fff;margin-bottom:0.4rem;">
                            "Paper wristbands"
                        </h3>
                        <p style="font-size:0.85rem;color:var(--text-secondary);line-height:1.5;">
                            "Tear, rip, disappear. No proof you attended a week later."
                        </p>
                    </div>

                    <div class="card" style="text-align:center;padding:1.5rem;">
                        <div class="landing-svg-icon icon-chart">
                            <svg viewBox="0 0 24 24">
                                <line x1="18" y1="20" x2="18" y2="10"/>
                                <line x1="12" y1="20" x2="12" y2="4"/>
                                <line x1="6" y1="20" x2="6" y2="14"/>
                            </svg>
                        </div>
                        <h3 style="font-size:0.95rem;font-weight:600;color:#fff;margin-bottom:0.4rem;">
                            "Spreadsheets"
                        </h3>
                        <p style="font-size:0.85rem;color:var(--text-secondary);line-height:1.5;">
                            "Manual entry, typos, and data that lives on someone's laptop."
                        </p>
                    </div>

                    <div class="card" style="text-align:center;padding:1.5rem;">
                        <div class="landing-svg-icon icon-proof">
                            <svg viewBox="0 0 24 24">
                                <path d="M12 22s8-4 8-10V5l-8-3-8 3v7c0 6 8 10 8 10z"/>
                            </svg>
                        </div>
                        <h3 style="font-size:0.95rem;font-weight:600;color:#fff;margin-bottom:0.4rem;">
                            "No-shows waste money"
                        </h3>
                        <p style="font-size:0.85rem;color:var(--text-secondary);line-height:1.5;">
                            "Registered attendees who don't show up cost organizers real money — food, swag, venue. There's no accountability."
                        </p>
                    </div>

                </div>
            </section>

            // ===== How It Works — Swimlane =====
            <section id="how-it-works" style="max-width:960px;margin:0 auto;padding:4rem 1.5rem;">
                <div style="text-align:center;margin-bottom:2rem;">
                    <h2 style="font-size:1.5rem;font-weight:700;color:#fff;margin-bottom:0.5rem;">
                        "How it works"
                    </h2>
                    <p style="color:var(--text-secondary);font-size:0.95rem;">
                        "Three perspectives. One seamless event."
                    </p>
                </div>

                // Role tabs
                <div style="display:flex;justify-content:center;gap:0.5rem;margin-bottom:2rem;">
                    {move || {
                        let active = active_role.get();
                        [SwimlaneRole::Organizer, SwimlaneRole::Staff, SwimlaneRole::Attendee].into_iter().map(|r| {
                            let is_active = r == active;
                            let accent = r.accent();
                            let bg = r.accent_bg();
                            let border = r.accent_border();
                            view! {
                                <button
                                    style=format!(
                                        "display:flex;align-items:center;gap:0.4rem;padding:0.5rem 1rem;border-radius:9999px;font-size:0.8rem;font-weight:600;border:1px solid {};cursor:pointer;transition:all 0.15s;background:{};color:{};",
                                        if is_active { border } else { "var(--border)" },
                                        if is_active { bg } else { "transparent" },
                                        if is_active { accent } else { "var(--text-secondary)" },
                                    )
                                    on:click=move |_| set_active_role.set(r)
                                >
                                    <span>{r.emoji()}</span>
                                    <span>{r.label()}</span>
                                </button>
                            }
                        }).collect_view()
                    }}
                </div>

                // Step flow indicators
                <div style="display:flex;align-items:center;justify-content:center;gap:0;margin-bottom:1.5rem;">
                    {move || {
                        let role = active_role.get();
                        let step = active_step.get();
                        let steps = role.steps();
                        let accent = role.accent();
                        steps.iter().enumerate().map(|(i, s)| {
                            let is_active = i == step;
                            let is_past = i < step;
                            let steps_len = steps.len();
                            view! {
                                <button
                                    style="display:flex;flex-direction:column;align-items:center;gap:0.35rem;background:none;border:none;cursor:pointer;padding:0;"
                                    on:click=move |_| set_active_step.set(i)
                                >
                                    <div style=format!(
                                        "width:2.5rem;height:2.5rem;border-radius:50%;display:flex;align-items:center;justify-content:center;font-size:0.85rem;transition:all 0.15s;border:2px solid {};background:{};",
                                        if is_active { accent } else if is_past { accent } else { "var(--border)" },
                                        if is_active { role.accent_bg().to_string() } else if is_past { "rgba(255,255,255,0.04)".to_string() } else { "transparent".to_string() }
                                    )>
                                        <span style=format!("color:{};", if is_active || is_past { accent } else { "var(--text-secondary)" })>{s.icon}</span>
                                    </div>
                                    <span style=format!(
                                        "font-size:0.65rem;font-weight:{};color:{};max-width:4.5rem;text-align:center;line-height:1.2;",
                                        if is_active { "600" } else { "500" },
                                        if is_active { "#fff" } else { "var(--text-secondary)" },
                                    )>{s.title}</span>
                                </button>
                                {if i < steps_len - 1 {
                                    let line_color = if is_past { accent } else { "var(--border)" }.to_string();
                                    view! {
                                        <div style=format!("width:2rem;height:2px;background:{};flex-shrink:0;", line_color)></div>
                                    }.into_any()
                                } else {
                                    ().into_any()
                                }}
                            }
                        }).collect_view()
                    }}
                </div>

                // Mockup card for active step
                <div style="max-width:340px;margin:0 auto;">
                    {move || swimlane_mockup(active_role.get(), active_step.get())}
                </div>

                // Mini swimlanes — all three roles compact
                <div style="margin-top:2.5rem;display:flex;flex-direction:column;gap:0.75rem;">
                    {move || {
                        let active = active_role.get();
                        [SwimlaneRole::Organizer, SwimlaneRole::Staff, SwimlaneRole::Attendee].into_iter().map(|r| {
                            let is_active = r == active;
                            let steps = r.steps();
                            let accent = r.accent();
                            let bg = r.accent_bg();
                            let border = r.accent_border();
                            view! {
                                <button
                                    style=format!(
                                        "display:flex;align-items:center;gap:0.75rem;padding:0.65rem 0.85rem;border-radius:var(--radius);border:1px solid {};background:{};cursor:pointer;width:100%;text-align:left;transition:all 0.15s;",
                                        if is_active { border } else { "var(--border)" },
                                        if is_active { bg } else { "transparent" },
                                    )
                                    on:click=move |_| {
                                        set_active_role.set(r);
                                        set_active_step.set(0);
                                    }
                                >
                                    <span style="font-size:0.75rem;font-weight:600;color:var(--text-secondary);min-width:5.5rem;">{format!("{} {}", r.emoji(), r.label())}</span>
                                    <div style="display:flex;align-items:center;gap:0.25rem;">
                                        {steps.iter().enumerate().map(|(i, s)| {
                                            view! {
                                                <>
                                                    <div style=format!(
                                                        "width:6px;height:6px;border-radius:50%;background:{};",
                                                        if is_active { accent } else { "var(--border)" },
                                                    ) title=s.title></div>
                                                    {if i < steps.len() - 1 {
                                                        view! {
                                                            <div style=format!(
                                                                "width:0.75rem;height:1px;background:{};",
                                                                if is_active { "var(--border)" } else { "var(--border)" },
                                                            )></div>
                                                        }.into_any()
                                                    } else {
                                                        ().into_any()
                                                    }}
                                                </>
                                            }
                                        }).collect_view()}
                                    </div>
                                    {if is_active {
                                        view! {
                                            <span style=format!("font-size:0.6rem;color:{};margin-left:auto;font-weight:600;", accent)>"viewing"</span>
                                        }.into_any()
                                    } else {
                                        ().into_any()
                                    }}
                                </button>
                            }
                        }).collect_view()
                    }}
                </div>

                // CTA
                <div style="text-align:center;margin-top:2rem;">
                    <A href="/demo" attr:class="btn btn-primary" attr:style="padding:0.75rem 1.5rem;">
                        "Try the full interactive demo \u{2192}"
                    </A>
                </div>
            </section>

            // ===== Waitlist =====
            <section id="waitlist" style="max-width:960px;margin:0 auto;padding:3rem 1.5rem 4rem;">
                <div style="text-align:center;max-width:480px;margin:0 auto;">
                    <h2 style="font-size:1.5rem;font-weight:700;color:#fff;margin-bottom:0.5rem;">
                        "Ready to end no-shows?"
                    </h2>
                    <p style="font-size:0.95rem;color:var(--text-secondary);margin-bottom:1.5rem;line-height:1.6;">
                        "Join the waitlist to bring deposit-backed events to your community."
                    </p>
                    <WaitlistForm />
                </div>
            </section>

            // ===== FAQ =====
            <section id="faq" style="max-width:720px;margin:0 auto;padding:3rem 1.5rem 4rem;">
                <div style="text-align:center;margin-bottom:2.5rem;">
                    <h2 style="font-size:1.5rem;font-weight:700;color:#fff;margin-bottom:0.5rem;">
                        "Frequently asked questions"
                    </h2>
                    <p style="color:var(--text-secondary);font-size:0.95rem;">
                        "Everything you need to know about BeThere."
                    </p>
                </div>

                <div style="display:flex;flex-direction:column;gap:0.75rem;">

                    // FAQ 1
                    <div style="background:var(--bg-secondary);border:1px solid var(--border);border-radius:var(--radius);padding:1.25rem 1.5rem;">
                        <h3 style="font-size:0.95rem;font-weight:600;color:#fff;margin-bottom:0.4rem;">
                            "What is BeThere?"
                        </h3>
                        <p style="font-size:0.85rem;color:var(--text-secondary);line-height:1.6;">
                            "BeThere is a deposit-backed event check-in platform built on Solana. Attendees put down a small deposit when they register. If they show up and complete a short quiz, they get their deposit back automatically — plus a compressed NFT badge as proof of attendance. If they don't show up, the organizer keeps the deposit."
                        </p>
                    </div>

                    // FAQ 2
                    <div style="background:var(--bg-secondary);border:1px solid var(--border);border-radius:var(--radius);padding:1.25rem 1.5rem;">
                        <h3 style="font-size:0.95rem;font-weight:600;color:#fff;margin-bottom:0.4rem;">
                            "Do attendees need a crypto wallet?"
                        </h3>
                        <p style="font-size:0.85rem;color:var(--text-secondary);line-height:1.6;">
                            "Not to check in! QR scanning works with any phone — no wallet required at the door. The wallet is only needed when claiming the NFT badge and deposit refund afterward. We support Phantom, Solflare, Backpack, or you can just paste your wallet address."
                        </p>
                    </div>

                    // FAQ 3
                    <div style="background:var(--bg-secondary);border:1px solid var(--border);border-radius:var(--radius);padding:1.25rem 1.5rem;">
                        <h3 style="font-size:0.95rem;font-weight:600;color:#fff;margin-bottom:0.4rem;">
                            "How does the deposit work?"
                        </h3>
                        <p style="font-size:0.85rem;color:var(--text-secondary);line-height:1.6;">
                            "Organizers set a deposit amount (e.g., 500 THB / ~$15). Attendees pay it when registering. After check-in and completing the quiz, the deposit is refunded on-chain as SOL + USDC directly to the attendee's wallet. No-shows forfeit their deposit to the organizer."
                        </p>
                    </div>

                    // FAQ 4
                    <div style="background:var(--bg-secondary);border:1px solid var(--border);border-radius:var(--radius);padding:1.25rem 1.5rem;">
                        <h3 style="font-size:0.95rem;font-weight:600;color:#fff;margin-bottom:0.4rem;">
                            "What is a compressed NFT badge?"
                        </h3>
                        <p style="font-size:0.85rem;color:var(--text-secondary);line-height:1.6;">
                            "It's a digital collectible on Solana that proves you attended an event. Unlike regular NFTs, compressed NFTs cost a fraction of a cent to mint (~$0.001) using Merkle trees. Each badge is unique to the event and lives in your wallet forever — think of it as a digital ticket stub that can't be faked."
                        </p>
                    </div>

                    // FAQ 5
                    <div style="background:var(--bg-secondary);border:1px solid var(--border);border-radius:var(--radius);padding:1.25rem 1.5rem;">
                        <h3 style="font-size:0.95rem;font-weight:600;color:#fff;margin-bottom:0.4rem;">
                            "What's the quiz?"
                        </h3>
                        <p style="font-size:0.85rem;color:var(--text-secondary);line-height:1.6;">
                            "Organizers can set a short quiz (e.g., 3-5 questions) about the event content. Attendees answer after check-in. It proves they actually paid attention — not just physically showed up. The passing threshold is configurable by the organizer."
                        </p>
                    </div>

                    // FAQ 6
                    <div style="background:var(--bg-secondary);border:1px solid var(--border);border-radius:var(--radius);padding:1.25rem 1.5rem;">
                        <h3 style="font-size:0.95rem;font-weight:600;color:#fff;margin-bottom:0.4rem;">
                            "How much does it cost for organizers?"
                        </h3>
                        <p style="font-size:0.85rem;color:var(--text-secondary);line-height:1.6;">
                            "BeThere is free during beta. We cover cNFT minting costs (fractions of a cent per badge). Future pricing will be per-event with a generous free tier. No per-attendee charge during beta."
                        </p>
                    </div>

                    // FAQ 7
                    <div style="background:var(--bg-secondary);border:1px solid var(--border);border-radius:var(--radius);padding:1.25rem 1.5rem;">
                        <h3 style="font-size:0.95rem;font-weight:600;color:#fff;margin-bottom:0.4rem;">
                            "Is BeThere only for crypto events?"
                        </h3>
                        <p style="font-size:0.85rem;color:var(--text-secondary);line-height:1.6;">
                            "It works great for any event! The deposit + check-in flow solves no-shows for meetups, workshops, conferences, and hackathons. The Solana/NFT part happens behind the scenes — attendees don't need to know anything about crypto."
                        </p>
                    </div>

                </div>

                // CTA under FAQ
                <div style="text-align:center;margin-top:2rem;">
                    <A href="/demo" attr:class="btn btn-primary" attr:style="padding:0.75rem 1.5rem;">
                        "Still curious? Try the demo →"
                    </A>
                </div>
            </section>

            // ===== Footer =====
            <footer class="landing-footer">
                <div class="landing-footer-grid">

                    // Column 1 — Brand
                    <div class="landing-footer-col">
                        <span style="font-weight:800;font-size:1.1rem;background:linear-gradient(135deg,#818cf8 0%,#6366f1 40%,#a78bfa 100%);-webkit-background-clip:text;-webkit-text-fill-color:transparent;background-clip:text;">
                            "BeThere"
                        </span>
                        <div class="landing-footer-brand-tagline">
                            "Show up. Get refunded."
                        </div>
                        <div class="landing-footer-brand-tagline" style="margin-top:0.5rem;">
                            "Built with "
                            <span style="color:var(--accent);">"🦀"</span>
                            " Rust & Solana"
                        </div>
                    </div>

                    // Column 2 — Product
                    <div class="landing-footer-col">
                        <h4>"Product"</h4>
                        <a href="#features">"Features"</a>
                        <a href="#how-it-works">"How It Works"</a>
                        <A href="/demo">"Try Demo"</A>
                        <A href="/login">"Sign In"</A>
                    </div>

                    // Column 3 — Community
                    <div class="landing-footer-col">
                        <h4>"Community"</h4>
                        <a href="https://x.com/ozoneRatchapon" target="_blank" rel="noopener noreferrer">"X / Twitter"</a>
                        <a href="https://github.com/solana-thailand" target="_blank" rel="noopener noreferrer">"GitHub"</a>
                        <a href="https://github.com/solana-thailand/BeThere" target="_blank" rel="noopener noreferrer">"Source Code"</a>
                    </div>

                </div>

                // Bottom row
                <div class="landing-footer-bottom">
                    <span class="landing-footer-copy">"© 2025 BeThere. All rights reserved."</span>
                    <span class="landing-footer-powered">
                        <svg viewBox="0 0 397 311" xmlns="http://www.w3.org/2000/svg">
                            <path d="M64.6 237.9c2.4-2.4 5.7-3.8 9.2-3.8h317.4c5.8 0 8.7 7 4.6 11.1l-62.7 62.7c-2.4 2.4-5.7 3.8-9.2 3.8H6.5c-5.8 0-8.7-7-4.6-11.1l62.7-62.7z" fill="currentColor"/>
                            <path d="M64.6 3.8C67.1 1.4 70.4 0 73.8 0h317.4c5.8 0 8.7 7 4.6 11.1l-62.7 62.7c-2.4 2.4-5.7 3.8-9.2 3.8H6.5c-5.8 0-8.7-7-4.6-11.1L64.6 3.8z" fill="currentColor"/>
                            <path d="M333.1 120.1c-2.4-2.4-5.7-3.8-9.2-3.8H6.5c-5.8 0-8.7 7-4.6 11.1l62.7 62.7c2.4 2.4 5.7 3.8 9.2 3.8h317.4c5.8 0 8.7-7 4.6-11.1l-62.7-62.7z" fill="currentColor"/>
                        </svg>
                        "Powered by Solana"
                    </span>
                </div>
            </footer>

        </div>
    }
}
