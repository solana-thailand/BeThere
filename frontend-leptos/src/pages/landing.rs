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

/// Landing page component.
#[component]
pub fn Landing() -> impl IntoView {
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
                        <a href="#organizers">"Organizers"</a>
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
                    <A href="/login" attr:class="btn btn-primary" attr:style="padding:0.85rem 2rem;font-size:1rem;">
                        "Get Started"
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

            // ===== How It Works =====
            <section id="how-it-works" style="max-width:960px;margin:0 auto;padding:4rem 1.5rem;">
                <div style="text-align:center;margin-bottom:3rem;">
                    <h2 style="font-size:1.5rem;font-weight:700;color:#fff;margin-bottom:0.5rem;">
                        "How it works"
                    </h2>
                    <p style="color:var(--text-secondary);font-size:0.95rem;">
                        "Four steps. Under a minute."
                    </p>
                </div>
                <div style="display:grid;grid-template-columns:repeat(auto-fit,minmax(200px,1fr));gap:1.25rem;">

                    <div style="background:var(--bg-secondary);border:1px solid var(--border);border-radius:var(--radius);padding:1.75rem 1.25rem;text-align:center;">
                        <div style="width:3.5rem;height:3.5rem;border-radius:50%;background:linear-gradient(135deg,rgba(99,102,241,0.2),rgba(129,140,248,0.1));display:inline-flex;align-items:center;justify-content:center;font-size:1.25rem;margin-bottom:1rem;border:1px solid rgba(99,102,241,0.3);">
                            "1"
                        </div>
                        <h3 style="font-size:1rem;font-weight:600;color:#fff;margin-bottom:0.5rem;">
                            "Put Down a Deposit"
                        </h3>
                        <p style="font-size:0.85rem;color:var(--text-secondary);line-height:1.5;">
                            "Reserve your spot with a small deposit. It's safely held until the event is over."
                        </p>
                    </div>

                    <div style="background:var(--bg-secondary);border:1px solid var(--border);border-radius:var(--radius);padding:1.75rem 1.25rem;text-align:center;">
                        <div style="width:3.5rem;height:3.5rem;border-radius:50%;background:linear-gradient(135deg,rgba(34,197,94,0.2),rgba(34,197,94,0.1));display:inline-flex;align-items:center;justify-content:center;font-size:1.25rem;margin-bottom:1rem;border:1px solid rgba(34,197,94,0.3);">
                            "2"
                        </div>
                        <h3 style="font-size:1rem;font-weight:600;color:#fff;margin-bottom:0.5rem;">
                            "Show Up & Scan"
                        </h3>
                        <p style="font-size:0.85rem;color:var(--text-secondary);line-height:1.5;">
                            "Get your QR scanned at the door. That's it — you're checked in and your deposit is marked for return."
                        </p>
                    </div>

                    <div style="background:var(--bg-secondary);border:1px solid var(--border);border-radius:var(--radius);padding:1.75rem 1.25rem;text-align:center;">
                        <div style="width:3.5rem;height:3.5rem;border-radius:50%;background:linear-gradient(135deg,rgba(245,158,11,0.2),rgba(245,158,11,0.1));display:inline-flex;align-items:center;justify-content:center;font-size:1.25rem;margin-bottom:1rem;border:1px solid rgba(245,158,11,0.3);">
                            "3"
                        </div>
                        <h3 style="font-size:1rem;font-weight:600;color:#fff;margin-bottom:0.5rem;">
                            "Complete the Quiz"
                        </h3>
                        <p style="font-size:0.85rem;color:var(--text-secondary);line-height:1.5;">
                            "Answer a few questions about the event content. Prove you actually engaged — not just physically showed up."
                        </p>
                    </div>

                    <div style="background:var(--bg-secondary);border:1px solid var(--border);border-radius:var(--radius);padding:1.75rem 1.25rem;text-align:center;">
                        <div style="width:3.5rem;height:3.5rem;border-radius:50%;background:linear-gradient(135deg,rgba(167,139,250,0.2),rgba(167,139,250,0.1));display:inline-flex;align-items:center;justify-content:center;font-size:1.25rem;margin-bottom:1rem;border:1px solid rgba(167,139,250,0.3);">
                            "4"
                        </div>
                        <h3 style="font-size:1rem;font-weight:600;color:#fff;margin-bottom:0.5rem;">
                            "Get Your Money Back + A Badge"
                        </h3>
                        <p style="font-size:0.85rem;color:var(--text-secondary);line-height:1.5;">
                            "Your deposit returns to your wallet automatically. You also get a digital badge — yours forever, proof you were there."
                        </p>
                    </div>

                </div>
            </section>

            // ===== For Organizers & Attendees =====
            <section id="organizers" style="max-width:960px;margin:0 auto;padding:4rem 1.5rem;">
                <div style="display:grid;grid-template-columns:repeat(auto-fit,minmax(300px,1fr));gap:1.5rem;">

                    // Organizers
                    <div style="background:var(--bg-secondary);border:1px solid var(--border);border-radius:var(--radius);padding:2rem;">
                        <div class="landing-svg-icon" style="width:36px;height:36px;margin-bottom:0.75rem;background:rgba(99,102,241,0.12);">
                            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                                <path d="M4 15s1-1 4-1 5 2 8 2 4-1 4-1V3s-1 1-4 1-5-2-8-2-4 1-4 1z"></path>
                                <line x1="4" y1="22" x2="4" y2="15"></line>
                            </svg>
                        </div>
                        <h2 style="font-size:1.25rem;font-weight:700;color:#fff;margin-bottom:0.5rem;">
                            "For Organizers"
                        </h2>
                        <p style="font-size:0.9rem;color:var(--text-secondary);line-height:1.6;margin-bottom:1.5rem;">
                            "Require a deposit to register. No-shows lose theirs — covering your costs. Staff scan QR codes with any phone. Real-time dashboard shows who showed up."
                        </p>
                        <A href="/login" attr:class="btn btn-primary">
                            "Start Your Event"
                        </A>
                    </div>

                    // Attendees
                    <div style="background:var(--bg-secondary);border:1px solid var(--border);border-radius:var(--radius);padding:2rem;">
                        <div class="landing-svg-icon" style="width:36px;height:36px;margin-bottom:0.75rem;background:rgba(167,139,250,0.12);">
                            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                                <polygon points="12 2 15.09 8.26 22 9.27 17 14.14 18.18 21.02 12 17.77 5.82 21.02 7 14.14 2 9.27 8.91 8.26 12 2"></polygon>
                            </svg>
                        </div>
                        <h2 style="font-size:1.25rem;font-weight:700;color:#fff;margin-bottom:0.5rem;">
                            "For Attendees"
                        </h2>
                        <p style="font-size:0.9rem;color:var(--text-secondary);line-height:1.6;margin-bottom:1.5rem;">
                            "Put down a small deposit, show up, complete a quick quiz about the event, and get it all back. Keep the digital badge as proof. Collect badges from every event you attend. The only risk? Not showing up — or not paying attention."
                        </p>
                        <a href="#how-it-works" class="btn btn-outline">
                            "Learn More"
                        </a>
                    </div>

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
                        <A href="/login">"Claim Badge"</A>
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
