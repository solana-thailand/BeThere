//! The `/sandbox` page: one card per step, one button at a time.

use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_meta::Title;

use super::burner::{burner_address, burner_supported, forget_burner};
use super::signer::{Signer, connect, sign_and_send};
use super::steps::{STEPS, Step, countdown};
use crate::api::{
    SandboxConfig, SandboxEvent, sandbox_check_in, sandbox_config, sandbox_create_event,
    sandbox_deposit_tx, sandbox_faucet, sandbox_refund_tx, sandbox_return_tx,
};
use crate::i18n::td_string;
use crate::locale::tr;

/// Seconds past `event_end` before the refund button opens: the cluster clock
/// can trail the browser's by a few seconds.
const REFUND_MARGIN_S: i64 = 5;

#[derive(Clone)]
enum Load {
    Loading,
    Off,
    Unsupported,
    Ready(SandboxConfig),
}

fn explorer_tx(signature: &str) -> String {
    format!("https://explorer.solana.com/tx/{signature}?cluster=devnet")
}

fn explorer_address(address: &str) -> String {
    format!("https://explorer.solana.com/address/{address}?cluster=devnet")
}

fn short(address: &str) -> String {
    match address.len() {
        n if n > 12 => format!("{}…{}", &address[..4], &address[n - 4..]),
        _ => address.to_string(),
    }
}

fn now_s() -> i64 {
    (js_sys::Date::now() / 1000.0) as i64
}

/// Run `step` against the API (and `signer` for the visitor's own
/// transactions). Returns the transaction signature.
async fn perform(
    step: Step,
    signer: Signer,
    rpc: String,
    wallet: String,
    event: Option<SandboxEvent>,
    set_event: WriteSignal<Option<SandboxEvent>>,
) -> Result<String, String> {
    let event_id = event.map(|e| e.event_id).unwrap_or_default();
    match step {
        Step::Event => {
            let created = sandbox_create_event().await?;
            let signature = created.signature.clone();
            set_event.set(Some(created));
            Ok(signature)
        }
        Step::Faucet => Ok(sandbox_faucet(&wallet).await?.signature),
        Step::Deposit => {
            let tx = sandbox_deposit_tx(&event_id, &wallet).await?;
            sign_and_send(&signer, &rpc, &tx.transaction_b64).await
        }
        Step::CheckIn => Ok(sandbox_check_in(&event_id, &wallet).await?.signature),
        Step::Refund => {
            let tx = sandbox_refund_tx(&event_id, &wallet).await?;
            sign_and_send(&signer, &rpc, &tx.transaction_b64).await
        }
        Step::Return => {
            let tx = sandbox_return_tx(&wallet).await?;
            sign_and_send(&signer, &rpc, &tx.transaction_b64).await
        }
        Step::Done => Err("nothing left to do".to_string()),
    }
}

#[component]
pub fn Sandbox() -> impl IntoView {
    let load = RwSignal::new(Load::Loading);
    let wallet = RwSignal::new(String::new());
    let step = RwSignal::new(Step::Event);
    let busy = RwSignal::new(false);
    let error = RwSignal::new(None::<String>);
    let (event, set_event) = signal(None::<SandboxEvent>);
    let signatures = RwSignal::new([const { None::<String> }; 6]);
    let now = RwSignal::new(now_s());
    let signer = RwSignal::new(Signer::Burner);
    let detected = RwSignal::new(Vec::<String>::new());

    spawn_local(async move {
        let config = match sandbox_config().await {
            Ok(config) if config.enabled => config,
            _ => return load.set(Load::Off),
        };
        if !burner_supported().await {
            return load.set(Load::Unsupported);
        }
        match burner_address().await {
            Ok(address) => {
                wallet.set(address);
                detected.set(crate::pages::deposit::js_interop::get_detected_wallets());
                load.set(Load::Ready(config));
            }
            Err(_) => load.set(Load::Unsupported),
        }
    });

    if let Ok(handle) =
        set_interval_with_handle(move || now.set(now_s()), std::time::Duration::from_secs(1))
    {
        on_cleanup(move || handle.clear());
    }

    let rpc = move || match load.get() {
        Load::Ready(config) => config.browser_rpc,
        _ => String::new(),
    };
    let refund_opens = move || event.get().map(|e| e.event_end + REFUND_MARGIN_S);
    let waiting =
        move || step.get() == Step::Refund && refund_opens().is_some_and(|open| now.get() < open);

    let run = move |which: Step| {
        busy.set(true);
        error.set(None);
        let (rpc, wallet, event) = (rpc(), wallet.get(), event.get());
        let who = signer.get();
        spawn_local(async move {
            match perform(which, who, rpc, wallet, event, set_event).await {
                Ok(signature) => {
                    signatures.update(|s| s[which.index()] = Some(signature));
                    step.set(which.next());
                }
                Err(message) => error.set(Some(message)),
            }
            busy.set(false);
        });
    };

    // Before the first step only: which wallet signs.
    let not_started = move || step.get() == Step::Event && signatures.with(|s| s[0].is_none());
    let use_burner = move |_| {
        signer.set(Signer::Burner);
        error.set(None);
        spawn_local(async move {
            if let Ok(address) = burner_address().await {
                wallet.set(address);
            }
        });
    };
    let use_wallet = move |name: String| {
        busy.set(true);
        error.set(None);
        spawn_local(async move {
            match connect(&name).await {
                Ok(address) => {
                    wallet.set(address);
                    signer.set(Signer::Wallet(name));
                }
                Err(message) => error.set(Some(message)),
            }
            busy.set(false);
        });
    };

    let start_over = move |_| {
        // A fresh test wallet, so the faucet's one-grant-per-wallet allows
        // another run; a real wallet stays the visitor's choice.
        if signer.get_untracked() == Signer::Burner {
            forget_burner();
            spawn_local(async move {
                if let Ok(address) = burner_address().await {
                    wallet.set(address);
                }
            });
        }
        set_event.set(None);
        signatures.set([const { None }; 6]);
        error.set(None);
        step.set(Step::Event);
    };

    let step_card = move |which: Step| {
        let state = move || match step.get() {
            current if current == which => "sandbox-step--current",
            current if current > which => "sandbox-step--done",
            _ => "sandbox-step--later",
        };
        view! {
            <li class=move || format!("sandbox-step {}", state())>
                <span class="sandbox-step-num" aria-hidden="true">{which.index() + 1}</span>
                <div class="sandbox-step-body">
                    <h2 class="sandbox-step-title">{tr(which.title())}</h2>
                    <p class="sandbox-step-hint">{tr(which.hint())}</p>
                    {move || (which == Step::Refund && step.get() == Step::Refund).then(|| {
                        view! {
                            <p class="sandbox-countdown" aria-live="polite">
                                {move || match refund_opens() {
                                    Some(open) if now.get() < open => view! {
                                        {tr(|l| td_string!(l, sandbox.ends_in))}
                                        " "
                                        {countdown(open - now.get())}
                                    }.into_any(),
                                    _ => tr(|l| td_string!(l, sandbox.ended)).into_any(),
                                }}
                            </p>
                        }
                    })}
                    {move || (step.get() == which).then(|| view! {
                        <button
                            class="btn btn-primary sandbox-action"
                            disabled=move || busy.get() || waiting()
                            on:click=move |_| run(which)
                        >
                            {move || match busy.get() {
                                true => tr(|l| td_string!(l, sandbox.working)).into_any(),
                                false => tr(which.button()).into_any(),
                            }}
                        </button>
                    })}
                    {move || signatures.get()[which.index()].clone().map(|signature| view! {
                        <a class="sandbox-tx" href=explorer_tx(&signature) target="_blank" rel="noopener">
                            {tr(|l| td_string!(l, sandbox.view_tx))}
                            " ↗"
                        </a>
                    })}
                </div>
            </li>
        }
    };

    view! {
        <Title text=tr(|l| td_string!(l, sandbox.page_title)) />
        <div class="sandbox-page">
            <a href="/" class="faq-back">{tr(|l| td_string!(l, sandbox.back_home))}</a>
            <span class="sandbox-badge">{tr(|l| td_string!(l, sandbox.devnet_badge))}</span>
            <h1 class="sandbox-title">{tr(|l| td_string!(l, sandbox.title))}</h1>
            <p class="sandbox-subtitle">{tr(|l| td_string!(l, sandbox.subtitle))}</p>
            {move || match load.get() {
                Load::Loading => view! { <p class="sandbox-note">{tr(|l| td_string!(l, sandbox.working))}</p> }.into_any(),
                Load::Off => view! {
                    <div class="sandbox-off">
                        <h2>{tr(|l| td_string!(l, sandbox.off_title))}</h2>
                        <p>{tr(|l| td_string!(l, sandbox.off_body))}</p>
                    </div>
                }.into_any(),
                Load::Unsupported => view! {
                    <p class="sandbox-error" role="alert">{tr(|l| td_string!(l, sandbox.unsupported))}</p>
                }.into_any(),
                Load::Ready(_) => view! {
                    {move || not_started().then(|| view! {
                        <div class="sandbox-signer" role="group" aria-label=tr(|l| td_string!(l, sandbox.signer_title))>
                            <h2 class="sandbox-signer-title">{tr(|l| td_string!(l, sandbox.signer_title))}</h2>
                            <button
                                class="sandbox-signer-opt"
                                class:sandbox-signer-opt--on=move || signer.get() == Signer::Burner
                                aria-pressed=move || (signer.get() == Signer::Burner).to_string()
                                disabled=move || busy.get()
                                on:click=use_burner
                            >
                                {tr(|l| td_string!(l, sandbox.signer_burner))}
                            </button>
                            {move || detected.get().into_iter().map(|name| {
                                let mine = name.clone();
                                let on = move || signer.get() == Signer::Wallet(mine.clone());
                                let pick = name.clone();
                                view! {
                                    <button
                                        class="sandbox-signer-opt"
                                        class:sandbox-signer-opt--on=on.clone()
                                        aria-pressed=move || on().to_string()
                                        disabled=move || busy.get()
                                        on:click=move |_| use_wallet(pick.clone())
                                    >
                                        {tr(|l| td_string!(l, sandbox.signer_use))}
                                        " "
                                        {name}
                                    </button>
                                }
                            }).collect::<Vec<_>>()}
                            {move || detected.with(Vec::is_empty).then(|| view! {
                                <p class="sandbox-note">{tr(|l| td_string!(l, sandbox.signer_none))}</p>
                            })}
                            <p class="sandbox-fineprint">{tr(|l| td_string!(l, sandbox.signer_why))}</p>
                        </div>
                    })}
                    <p class="sandbox-note">
                        {move || match signer.get() {
                            Signer::Burner => tr(|l| td_string!(l, sandbox.notice)).into_any(),
                            Signer::Wallet(_) => tr(|l| td_string!(l, sandbox.notice_own)).into_any(),
                        }}
                    </p>
                    <p class="sandbox-wallet">
                        {move || match signer.get() {
                            Signer::Burner => tr(|l| td_string!(l, sandbox.wallet_label)).into_any(),
                            Signer::Wallet(_) => tr(|l| td_string!(l, sandbox.wallet_label_own)).into_any(),
                        }}
                        ": "
                        <a href=move || explorer_address(&wallet.get()) target="_blank" rel="noopener">
                            <code>{move || short(&wallet.get())}</code>
                        </a>
                    </p>
                    <ol class="sandbox-steps">
                        {STEPS.into_iter().map(step_card).collect::<Vec<_>>()}
                    </ol>
                    {move || error.get().map(|message| view! {
                        <p class="sandbox-error" role="alert">{message}</p>
                    })}
                    {move || (step.get() == Step::Done).then(|| view! {
                        <div class="sandbox-done">
                            <h2>{tr(|l| td_string!(l, sandbox.done_title))}</h2>
                            <p>{tr(|l| td_string!(l, sandbox.done_body))}</p>
                        </div>
                    })}
                    {move || (step.get() != Step::Event || error.get().is_some()).then(|| view! {
                        <button class="btn btn-outline sandbox-restart" on:click=start_over>
                            {tr(|l| td_string!(l, sandbox.start_over))}
                        </button>
                    })}
                    {move || matches!(signer.get(), Signer::Wallet(_)).then(|| view! {
                        <p class="sandbox-fineprint">
                            {tr(|l| td_string!(l, sandbox.faucet_more))}
                            " "
                            <a href="https://faucet.solana.com" target="_blank" rel="noopener">{tr(|l| td_string!(l, sandbox.faucet_sol))}</a>
                            " · "
                            <a href="https://faucet.circle.com" target="_blank" rel="noopener">{tr(|l| td_string!(l, sandbox.faucet_usdc))}</a>
                        </p>
                    })}
                    <p class="sandbox-fineprint">{tr(|l| td_string!(l, sandbox.guard_note))}</p>
                }.into_any(),
            }}
        </div>
    }
}
