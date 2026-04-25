//! Landing page — public marketing page for BeThere.
//!
//! Showcases the platform with hero, problem/solution, how-it-works steps,
//! organizer and attendee pitches, and footer branding.
//! No backend calls — purely static marketing content with SPA navigation.

use leptos::prelude::*;
use leptos_router::components::A;

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
                    <div style="display:flex;align-items:center;gap:0.75rem;">
                        <A href="/login" attr:class="btn btn-outline btn-sm">
                            "Sign In"
                        </A>
                    </div>
                </div>
            </nav>

            // ===== Hero =====
            <section style="max-width:960px;margin:0 auto;padding:5rem 1.5rem 4rem;text-align:center;">
                <div style="font-size:3rem;margin-bottom:1rem;">"🎫"</div>
                <h1 style="font-size:clamp(1.75rem,5vw,2.75rem);font-weight:800;line-height:1.15;margin-bottom:1.25rem;color:#fff;">
                    "Check in. Mint."
                    <br />
                    <span style="background:linear-gradient(135deg,#818cf8,#6366f1,#a78bfa);-webkit-background-clip:text;-webkit-text-fill-color:transparent;background-clip:text;">
                        "Prove you were there."
                    </span>
                </h1>
                <p style="font-size:1.1rem;color:var(--text-secondary);max-width:520px;margin:0 auto 2.25rem;line-height:1.6;">
                    "Solana-powered event check-ins with compressed NFTs as proof of attendance. No more lost wristbands. No more forgotten spreadsheets."
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

            // ===== Problem =====
            <section style="max-width:960px;margin:0 auto;padding:3rem 1.5rem 4rem;">
                <div style="text-align:center;margin-bottom:2.5rem;">
                    <h2 style="font-size:1.5rem;font-weight:700;color:#fff;margin-bottom:0.5rem;">
                        "Attendance tracking is broken"
                    </h2>
                    <p style="color:var(--text-secondary);font-size:0.95rem;">
                        "Paper gets lost. Spreadsheets get forgotten. Data stays siloed."
                    </p>
                </div>
                <div style="display:grid;grid-template-columns:repeat(auto-fit,minmax(240px,1fr));gap:1rem;">

                    <div class="card" style="text-align:center;padding:1.5rem;">
                        <div style="font-size:1.75rem;margin-bottom:0.75rem;">"📋"</div>
                        <h3 style="font-size:0.95rem;font-weight:600;color:#fff;margin-bottom:0.4rem;">
                            "Paper wristbands"
                        </h3>
                        <p style="font-size:0.85rem;color:var(--text-secondary);line-height:1.5;">
                            "Tear, rip, disappear. No proof you attended a week later."
                        </p>
                    </div>

                    <div class="card" style="text-align:center;padding:1.5rem;">
                        <div style="font-size:1.75rem;margin-bottom:0.75rem;">"📊"</div>
                        <h3 style="font-size:0.95rem;font-weight:600;color:#fff;margin-bottom:0.4rem;">
                            "Spreadsheets"
                        </h3>
                        <p style="font-size:0.85rem;color:var(--text-secondary);line-height:1.5;">
                            "Manual entry, typos, and data that lives on someone's laptop."
                        </p>
                    </div>

                    <div class="card" style="text-align:center;padding:1.5rem;">
                        <div style="font-size:1.75rem;margin-bottom:0.75rem;">"🤷"</div>
                        <h3 style="font-size:0.95rem;font-weight:600;color:#fff;margin-bottom:0.4rem;">
                            "No lasting proof"
                        </h3>
                        <p style="font-size:0.85rem;color:var(--text-secondary);line-height:1.5;">
                            "Attendance should be permanent, verifiable, and yours to keep."
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
                        "Three steps. Under a minute."
                    </p>
                </div>
                <div style="display:grid;grid-template-columns:repeat(auto-fit,minmax(240px,1fr));gap:1.5rem;">

                    <div style="background:var(--bg-secondary);border:1px solid var(--border);border-radius:var(--radius);padding:2rem 1.5rem;text-align:center;">
                        <div style="width:3.5rem;height:3.5rem;border-radius:50%;background:linear-gradient(135deg,rgba(99,102,241,0.2),rgba(129,140,248,0.1));display:inline-flex;align-items:center;justify-content:center;font-size:1.25rem;margin-bottom:1rem;border:1px solid rgba(99,102,241,0.3);">
                            "1"
                        </div>
                        <h3 style="font-size:1rem;font-weight:600;color:#fff;margin-bottom:0.5rem;">
                            "Register Event"
                        </h3>
                        <p style="font-size:0.85rem;color:var(--text-secondary);line-height:1.5;">
                            "Organizer creates an event and gets a unique check-in page with QR codes."
                        </p>
                    </div>

                    <div style="background:var(--bg-secondary);border:1px solid var(--border);border-radius:var(--radius);padding:2rem 1.5rem;text-align:center;">
                        <div style="width:3.5rem;height:3.5rem;border-radius:50%;background:linear-gradient(135deg,rgba(34,197,94,0.2),rgba(34,197,94,0.1));display:inline-flex;align-items:center;justify-content:center;font-size:1.25rem;margin-bottom:1rem;border:1px solid rgba(34,197,94,0.3);">
                            "2"
                        </div>
                        <h3 style="font-size:1rem;font-weight:600;color:#fff;margin-bottom:0.5rem;">
                            "Scan & Check In"
                        </h3>
                        <p style="font-size:0.85rem;color:var(--text-secondary);line-height:1.5;">
                            "Staff scans attendee QR codes — instant verification on Solana."
                        </p>
                    </div>

                    <div style="background:var(--bg-secondary);border:1px solid var(--border);border-radius:var(--radius);padding:2rem 1.5rem;text-align:center;">
                        <div style="width:3.5rem;height:3.5rem;border-radius:50%;background:linear-gradient(135deg,rgba(167,139,250,0.2),rgba(167,139,250,0.1));display:inline-flex;align-items:center;justify-content:center;font-size:1.25rem;margin-bottom:1rem;border:1px solid rgba(167,139,250,0.3);">
                            "3"
                        </div>
                        <h3 style="font-size:1rem;font-weight:600;color:#fff;margin-bottom:0.5rem;">
                            "Claim NFT"
                        </h3>
                        <p style="font-size:0.85rem;color:var(--text-secondary);line-height:1.5;">
                            "Attendees claim a compressed NFT — permanent, verifiable proof of attendance."
                        </p>
                    </div>

                </div>
            </section>

            // ===== For Organizers & Attendees =====
            <section style="max-width:960px;margin:0 auto;padding:4rem 1.5rem;">
                <div style="display:grid;grid-template-columns:repeat(auto-fit,minmax(300px,1fr));gap:1.5rem;">

                    // Organizers
                    <div style="background:var(--bg-secondary);border:1px solid var(--border);border-radius:var(--radius);padding:2rem;">
                        <div style="font-size:1.5rem;margin-bottom:0.75rem;">"🎪"</div>
                        <h2 style="font-size:1.25rem;font-weight:700;color:#fff;margin-bottom:0.5rem;">
                            "For Organizers"
                        </h2>
                        <p style="font-size:0.9rem;color:var(--text-secondary);line-height:1.6;margin-bottom:1.5rem;">
                            "Set up check-in in minutes. Staff scan QR codes with any phone. Real-time dashboard shows who's here. No app downloads, no custom hardware."
                        </p>
                        <A href="/login" attr:class="btn btn-primary">
                            "Start Your Event"
                        </A>
                    </div>

                    // Attendees
                    <div style="background:var(--bg-secondary);border:1px solid var(--border);border-radius:var(--radius);padding:2rem;">
                        <div style="font-size:1.5rem;margin-bottom:0.75rem;">"✨"</div>
                        <h2 style="font-size:1.25rem;font-weight:700;color:#fff;margin-bottom:0.5rem;">
                            "For Attendees"
                        </h2>
                        <p style="font-size:0.9rem;color:var(--text-secondary);line-height:1.6;margin-bottom:1.5rem;">
                            "Get checked in with a scan, then claim a compressed NFT as proof you were there. Build an on-chain attendance portfolio across every event you attend."
                        </p>
                        <a href="#how-it-works" class="btn btn-outline">
                            "Learn More"
                        </a>
                    </div>

                </div>
            </section>

            // ===== Footer =====
            <footer style="border-top:1px solid var(--border);margin-top:2rem;">
                <div style="max-width:960px;margin:0 auto;padding:2rem 1.5rem;display:flex;flex-wrap:wrap;align-items:center;justify-content:space-between;gap:1rem;">
                    <div style="display:flex;align-items:center;gap:0.5rem;font-size:0.85rem;color:var(--text-muted);">
                        <span style="font-weight:700;background:linear-gradient(135deg,#818cf8,#6366f1);-webkit-background-clip:text;-webkit-text-fill-color:transparent;background-clip:text;">
                            "BeThere"
                        </span>
                        <span>"×"</span>
                        <span>"Solana Thailand"</span>
                    </div>
                    <div style="display:flex;align-items:center;gap:1.25rem;">
                        <a
                            href="https://github.com/solana-thailand"
                            target="_blank"
                            rel="noopener noreferrer"
                            style="color:var(--text-muted);font-size:0.8rem;text-decoration:none;transition:color 0.2s;"
                        >
                            "GitHub"
                        </a>
                        <span style="color:var(--text-muted);font-size:0.8rem;">
                            "Powered by Solana"
                        </span>
                    </div>
                </div>
            </footer>

        </div>
    }
}
