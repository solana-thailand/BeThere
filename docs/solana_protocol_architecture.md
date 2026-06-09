# BeThere — Solana Protocol Architecture Diagram

> Deposit-backed event check-in platform on Solana.
> Program ID: `C6HDeZES9aPpNwe3UvS9ecmfcRhH1XeJb8PGJmLG3z3T`
>
> **Companion document:** [Protocol POC Requirements](protocol_poc_requirements.md) — formal "shall" requirements for every instruction and account.

---

## 1. System Overview — High-Level Architecture

```mermaid
graph TB
    subgraph "User Layer"
        ATTENDEE["👤 Attendee<br/>(Browser + Wallet)"]
        ORGANIZER["Organizer / Admin<br/>(Browser Dashboard)"]
    end

    subgraph "Edge — Cloudflare Workers"
        FE["Leptos WASM<br/>CSR Frontend"]
        API["Axum API Router<br/>(Rust → WASM)"]
    end

    subgraph "Storage — Cloudflare"
        D1["D1 (SQLite)<br/>Primary Data Store"]
        KV["KV Namespace<br/>Cache + Fallback"]
        R2["R2 Bucket<br/>Slips, Refund Proofs,<br/>Badge SVGs"]
        DO["Durable Objects<br/>Per-Event ACID Locks"]
    end

    subgraph "Solana Blockchain"
        ESCROW["bethere-escrow<br/>Program (On-Chain)"]
        TOKEN["SPL Token Program"]
        ATA["Associated Token<br/>Program"]
        SYSTEM["System Program"]
    end

    subgraph "External Services"
        HELIUS["Helius RPC<br/>+ DAS API<br/>+ Webhooks"]
        GOOGLE["Google Sheets<br/>(Attendee Registry)"]
        OAUTH["Google OAuth<br/>(Auth)"]
    end

    ATTENDEE --> FE
    ORGANIZER --> FE
    FE --> API
    API --> D1
    API --> KV
    API --> R2
    API --> DO
    API --> HELIUS
    API --> GOOGLE
    API --> OAUTH
    HELIUS -->|"Transaction<br/>Confirmation"| ESCROW
    ESCROW --> TOKEN
    ESCROW --> ATA
    ESCROW --> SYSTEM
    API -->|"Build TX<br/>(Solana Pay)"| ATTENDEE
    ATTENDEE -->|"Sign TX"| ESCROW
```

---

## 2. On-Chain Program — Account Structure

```mermaid
graph TB
    subgraph "bethere-escrow Program<br/>C6HDeZES9aPpNwe3..."
        direction TB

        EE["<b>EventEscrow PDA</b><br/>─────────────────<br/>seeds: [escrow, organizer, event_id]<br/>owner: bethere-escrow<br/>space: 192 bytes<br/>─────────────────<br/>version: u8<br/>organizer: Pubkey<br/>event_id: u64<br/>deposit_mint: Pubkey (USDC)<br/>vault: Pubkey (ATA address)<br/>deposit_amount: u64<br/>event_end: i64<br/>refund_deadline: i64<br/>total_deposited: u64<br/>total_refunded: u64<br/>total_forfeited: u64<br/>is_active: bool<br/>bump: u8"]

        VAULT["<b>Vault — Associated Token Account</b><br/>─────────────────<br/>mint: USDC<br/>authority: EventEscrow PDA<br/>owner: SPL Token Program"]

        AD["<b>AttendeeDeposit PDA</b><br/>─────────────────<br/>seeds: [deposit, event, attendee]<br/>owner: bethere-escrow<br/>space: 96 bytes<br/>─────────────────<br/>version: u8<br/>attendee: Pubkey<br/>event: Pubkey (EventEscrow)<br/>amount: u64<br/>deposited_at: i64<br/>checked_in: bool<br/>refunded: bool<br/>bump: u8"]
    end

    EE -->|"owns"| VAULT
    EE -->|"references"| AD

    style EE fill:#6366f1,stroke:#818cf8,color:#fff
    style VAULT fill:#14f195,stroke:#22c55e,color:#000
    style AD fill:#9945ff,stroke:#a78bfa,color:#fff
```

---

## 3. Program Instructions — Lifecycle Flow

```mermaid
stateDiagram-v2
    [*] --> Created: create_event<br/>(Organizer)
    Created --> Active: is_active = true<br/>Accepting deposits

    Active --> Deposited: deposit<br/>(Attendee signs)<br/>USDC → Vault
    Deposited --> Deposited: More deposits...
    Active --> Deactivated: deactivate_event<br/>(Organizer)
    Deactivated --> Deposited: Existing deposits<br/>remain

    Deposited --> CheckedIn: mark_checked_in<br/>(Organizer signs)
    CheckedIn --> Refunded: refund<br/>(Attendee signs)<br/>Vault → Attendee
    Deposited --> Refunded: refund<br/>(No-show, after event_end)<br/>Anti-rug-pull

    Refunded --> ClosedDeposit: close_deposit<br/>(Anyone, reclaim rent)
    Deactivated --> Claimed: claim_forfeited<br/>(Organizer, after deadline)<br/>No-show funds → Organizer
    Claimed --> ClosedEvent: close_event<br/>(Organizer, reclaim rent)

    ClosedDeposit --> [*]
    ClosedEvent --> [*]

    note right of Active: In-Person & Hybrid only
    note right of Refunded: refund_and_close<br/>= atomic combo
    note right of Claimed: After refund_deadline<br/>Only unchecked-in deposits
```

---

## 4. In-Person Attendee Flow — Full Sequence

```mermaid
sequenceDiagram
    participant A as Attendee
    participant FE as Frontend (WASM)
    participant W as Cloudflare Worker
    participant G as Google Sheets
    participant H as Helius RPC
    participant S as Solana (Escrow)

    Note over A,S: Phase 1 — Event Setup
    W->>G: Read attendee list
    W->>H: Build init_escrow TX
    H->>S: create_event instruction
    S-->>H: EventEscrow PDA + Vault ATA created
    H-->>W: TX confirmed

    Note over A,S: Phase 2 — Deposit
    A->>FE: Visit event page, connect wallet
    FE->>W: POST /api/deposit/usdc
    W->>W: Derive EventEscrow PDA<br/>Derive AttendeeDeposit PDA<br/>Derive Vault ATA
    W->>W: Build serialized TX (base64)
    W-->>FE: Solana Pay URL + TX
    FE->>A: Show QR / Wallet prompt
    A->>S: Sign + Send deposit TX
    S->>S: Validate: is_active,<br/>correct amount
    S-->>A: AttendeeDeposit PDA created<br/>USDC transferred to Vault

    Note over A,S: Phase 3 — Event Day Check-in
    A->>FE: Show QR code
    FE->>W: Organizer scans QR
    W->>H: Build mark_checked_in TX
    W-->>FE: TX for organizer to sign
    FE->>S: Organizer signs mark_checked_in
    S->>S: Set checked_in = true
    S-->>FE: Checked in ✓

    Note over A,S: Phase 4 — Claim NFT
    A->>FE: Click "Claim NFT"
    FE->>W: POST /api/claim/{token}
    W->>W: Validate: checked_in,<br/>not already claimed
    W->>H: mintCompressedNft (Helius DAS)
    H->>S: Mint cNFT to attendee wallet
    W->>G: Mark claimed in sheet
    W-->>FE: NFT minted ✓

    Note over A,S: Phase 5 — Refund
    A->>FE: Click "Refund"
    FE->>W: POST /api/deposit/usdc/refund
    W->>W: Build refund_and_close TX
    W-->>FE: TX for attendee to sign
    FE->>S: Sign refund + close_deposit
    S->>S: Vault → Attendee USDC ATA<br/>Close AttendeeDeposit PDA
    S-->>A: USDC returned + rent reclaimed
```

---

## 5. Online Attendee Flow — Quest-Based Verification

```mermaid
flowchart TD
    RSVP["RSVP (off-chain)<br/>No deposit, no wallet"] --> QUIZ["Quiz / Adventure<br/>Challenge Gate"]
    QUIZ --> |"Pass"| PASS["Attendance Verified ✓<br/>(off-chain)"]
    QUIZ --> |"Fail"| RETRY["Retry<br/>(max attempts)"]
    RETRY --> QUIZ
    PASS --> CLAIM["Connect Wallet<br/>Claim cNFT"]
    CLAIM --> |"Eligible"| NFT["Compressed NFT Minted<br/>(Helius DAS)"]
    CLAIM --> |"Not eligible"| BLOCKED["Complete quest first"]
    NFT --> DONE["Done — Proof of<br/>Participation"]

    style RSVP fill:#3b82f6,stroke:#60a5fa,color:#fff
    style NFT fill:#14f195,stroke:#22c55e,color:#000
    style BLOCKED fill:#ef4444,stroke:#f87171,color:#fff
```

---

## 6. Event Format — Decision Matrix

```mermaid
flowchart LR
    subgraph "Event Format"
        IP["In-Person"]
        ON["Online"]
        HY["Hybrid"]
    end

    subgraph "Deposit Required?"
        IP_D["✅ USDC Escrow<br/>mandatory"]
        ON_D["❌ No deposit<br/>Quest gates only"]
        HY_D["✅ In-person track: USDC<br/>Online track: None"]
    end

    subgraph "Check-in Method"
        IP_C["QR Scan<br/>(on-chain mark_checked_in)"]
        ON_C["Quiz / Adventure<br/>(off-chain)"]
        HY_C["Both tracks<br/>run in parallel"]
    end

    subgraph "NFT Claim Path"
        IP_N["After check-in"]
        ON_N["After quest pass"]
        HY_N["After respective<br/>track completion"]
    end

    subgraph "Escrow PDA"
        IP_E["✅ Created"]
        ON_E["❌ None"]
        HY_E["✅ Created<br/>(in-person deposits only)"]
    end

    IP --> IP_D --> IP_C --> IP_N --> IP_E
    ON --> ON_D --> ON_C --> ON_N --> ON_E
    HY --> HY_D --> HY_C --> HY_N --> HY_E

    style IP fill:#6366f1,stroke:#818cf8,color:#fff
    style ON fill:#3b82f6,stroke:#60a5fa,color:#fff
    style HY fill:#9945ff,stroke:#a78bfa,color:#fff
```

---

## 7. Escrow Indexer — On-Chain Event Sync

```mermaid
flowchart LR
    subgraph "Solana"
        TX["On-Chain TX<br/>(deposit, refund,<br/>claim_forfeited, etc.)"]
    end

    subgraph "Helius"
        WH["Helius Webhook<br/>(Enhanced TX)"]
        POLL["RPC Poller<br/>(getSignaturesForAddress)"]
    end

    subgraph "Worker — Escrow Indexer"
        PARSE["Parse Instruction<br/>Discriminator"]
        MAP["Map to<br/>EscrowInstruction"]
        STORE["Write OnChainEvent<br/>→ D1 (escrow_events)"]
        UPDATE["Update<br/>Deposit Status<br/>in D1"]
    end

    TX -->|"Webhook<br/>notification"| WH
    TX -->|"Fallback<br/>polling"| POLL
    WH --> PARSE
    POLL --> PARSE
    PARSE --> MAP
    MAP --> STORE
    MAP --> UPDATE

    style WH fill:#f59e0b,stroke:#eab308,color:#000
    style POLL fill:#f59e0b,stroke:#eab308,color:#000
    style STORE fill:#6366f1,stroke:#818cf8,color:#fff
    style UPDATE fill:#6366f1,stroke:#818cf8,color:#fff
```

---

## 8. Worker Module Architecture — Hexagonal Layers

```mermaid
graph TB
    subgraph "Frontend Layer"
        LEPTOS["Leptos CSR<br/>(WASM in Browser)"]
    end

    subgraph "HTTP Handler Layer"
        ROUTER["Axum Router<br/>/api/* routes"]
        MW["Middleware<br/>Auth, CORS, Rate Limit"]
        HANDLERS["Handlers<br/>register, checkin,<br/>claim, deposit,<br/>quiz, adventure, escrow"]
    end

    subgraph "Domain Layer (domain crate)"
        MODELS["Models<br/>Event, Attendee,<br/>Deposit, Adventure"]
        CONFIG["Config<br/>AppConfig, Solana,<br/>NFT, Sheets"]
        QR["QR<br/>Code Generation"]
    end

    subgraph "Infrastructure Layer"
        D1MOD["D1 Module<br/>SQL queries"]
        KVMOD["KV Module<br/>Key-value store"]
        SOLMOD["Solana Escrow<br/>TX Builders<br/>(crypto + wire)"]
        SOLRPC["Solana RPC<br/>NFT Minting<br/>Wallet Validation"]
        SHEETS["Google Sheets<br/>Read/Write"]
        INDEXER["Escrow Indexer<br/>Poller + Webhook"]
        DOCS["Durable Objects<br/>Claim Locks"]
        R2MOD["R2 Storage<br/>Slip Uploads"]
    end

    subgraph "External"
        D1DB["D1 Database"]
        KVS["KV Store"]
        SOLANA["Solana Blockchain"]
        HELIUS["Helius RPC/DAS"]
        GSHEETS["Google Sheets API"]
    end

    LEPTOS -->|"HTTP/JSON"| ROUTER
    ROUTER --> MW --> HANDLERS
    HANDLERS --> MODELS
    HANDLERS --> CONFIG
    HANDLERS --> D1MOD
    HANDLERS --> KVMOD
    HANDLERS --> SOLMOD
    HANDLERS --> SOLRPC
    HANDLERS --> SHEETS
    HANDLERS --> INDEXER
    HANDLERS --> DOCS
    HANDLERS --> R2MOD

    D1MOD --> D1DB
    KVMOD --> KVS
    SOLMOD --> SOLANA
    SOLRPC --> HELIUS
    SHEETS --> GSHEETS

    style LEPTOS fill:#9945ff,stroke:#a78bfa,color:#fff
    style ROUTER fill:#6366f1,stroke:#818cf8,color:#fff
    style MODELS fill:#14f195,stroke:#22c55e,color:#000
    style D1DB fill:#3b82f6,stroke:#60a5fa,color:#fff
    style SOLANA fill:#9945ff,stroke:#a78bfa,color:#fff
    style HELIUS fill:#f59e0b,stroke:#eab308,color:#000
```

---

## 9. PDA Derivation — Seed Hierarchy

```mermaid
graph TD
    ORG["Organizer Pubkey<br/>(base58)"]
    EID["event_id: u64<br/>(off-chain event ID)"]
    ATT["Attendee Pubkey<br/>(wallet address)"]

    subgraph "EventEscrow PDA"
        EE_SEED["seeds = [b'escrow',<br/>organizer,<br/>event_id_le]"]
        EE_ADDR["PDA Address (base58)<br/>192 bytes rent-exempt"]
        EE_VAULT["Vault ATA<br/>(Associated Token Account)<br/>mint: USDC<br/>authority: EventEscrow PDA"]
    end

    subgraph "AttendeeDeposit PDA"
        AD_SEED["seeds = [b'deposit',<br/>event_escrow_pubkey,<br/>attendee_pubkey]"]
        AD_ADDR["PDA Address (base58)<br/>96 bytes rent-exempt"]
    end

    ORG --> EE_SEED
    EID --> EE_SEED
    EE_SEED --> EE_ADDR
    EE_ADDR --> EE_VAULT

    EE_ADDR --> AD_SEED
    ATT --> AD_SEED
    AD_SEED --> AD_ADDR

    style EE_ADDR fill:#6366f1,stroke:#818cf8,color:#fff
    style EE_VAULT fill:#14f195,stroke:#22c55e,color:#000
    style AD_ADDR fill:#9945ff,stroke:#a78bfa,color:#fff
```

---

## 10. Security Model — Trust Boundaries

```mermaid
flowchart TB
    subgraph "Trustless — Protocol Guarantees"
        T1["Vault authority = EventEscrow PDA<br/>Only escrow program can move funds"]
        T2["Organizer cannot steal deposits<br/>Attendee self-refund after event_end"]
        T3["Refund_deadline is immutable<br/>set at create_event, never changes"]
        T4["checked_in flag set by organizer only<br/>has_one authority constraint"]
    end

    subgraph "Requires Trust — Off-Chain"
        R1["Organizer marks correct attendee<br/>checked_in (could mark wrong person)"]
        R2["THB refunds processed manually<br/>by organizer (off-chain)"]
        R3["NFT minting via Helius API<br/>cNFT state depends on Helius uptime"]
    end

    subgraph "Mitigations"
        M1["Anti-rug-pull: refund allowed<br/>after event_end regardless of check-in"]
        M2["Time-based refund eligibility<br/>checked on-chain"]
        M3["Escrow indexer verifies<br/>on-chain state independently"]
        M4["Schema versioning on<br/>EventEscrow and AttendeeDeposit"]
    end

    T1 --> M1
    R1 --> M2
    R3 --> M3
    T4 --> M4

    style T1 fill:#22c55e,stroke:#4ade80,color:#000
    style T2 fill:#22c55e,stroke:#4ade80,color:#000
    style T3 fill:#22c55e,stroke:#4ade80,color:#000
    style T4 fill:#22c55e,stroke:#4ade80,color:#000
    style R1 fill:#f59e0b,stroke:#eab308,color:#000
    style R2 fill:#f59e0b,stroke:#eab308,color:#000
    style R3 fill:#f59e0b,stroke:#eab308,color:#000
```

---

## 11. Dual-Track Payment — USDC vs THB

```mermaid
flowchart LR
    subgraph "USDC On-Chain"
        U1["Attendee connects<br/>Solana wallet"]
        U2["Worker builds TX<br/>(Solana Pay URL)"]
        U3["Attendee signs TX<br/>USDC → Vault ATA"]
        U4["Escrow Indexer<br/>confirms on-chain"]
        U5["Deposit verified ✓<br/>Refund: self-serve"]
    end

    subgraph "THB Off-Chain"
        T1["Attendee uploads<br/>PromptPay slip"]
        T2["Slip stored in R2<br/>Pending admin review"]
        T3["Admin verifies/rejects<br/>via dashboard"]
        T4["Deposit verified ✓<br/>Refund: manual"]
    end

    U1 --> U2 --> U3 --> U4 --> U5
    T1 --> T2 --> T3 --> T4

    style U3 fill:#14f195,stroke:#22c55e,color:#000
    style T2 fill:#f59e0b,stroke:#eab308,color:#000
```

---

## 12. Instruction Interaction Matrix — CPI & Data Flow

```mermaid
graph LR
    subgraph "Program Instructions"
        CREATE["create_event<br/>disc: 0"]
        DEPOSIT["deposit<br/>disc: 1"]
        CHECKIN["mark_checked_in<br/>disc: 2"]
        REFUND["refund<br/>disc: 3"]
        CLAIM["claim_forfeited<br/>disc: 4"]
        CLOSE["close_event<br/>disc: 5"]
        DEACTIVATE["deactivate_event<br/>disc: 6"]
        CLOSEDEP["close_deposit<br/>disc: 7"]
        ROLLOVER["rollover_deposit<br/>disc: 8"]
    end

    subgraph "CPI Targets"
        SPL["SPL Token<br/>Program"]
        SYS["System<br/>Program"]
        ASSOC["Associated Token<br/>Program"]
    end

    CREATE -->|"create ATA"| ASSOC
    CREATE -->|"alloc space"| SYS
    DEPOSIT -->|"transfer USDC"| SPL
    REFUND -->|"transfer USDC<br/>back to attendee"| SPL
    CLAIM -->|"transfer forfeited<br/>USDC to organizer"| SPL
    CLOSE -->|"reclaim rent"| SYS
    CLOSEDEP -->|"reclaim rent"| SYS
    ROLLOVER -->|"vault-to-vault<br/>atomic transfer"| SPL

    DEPOSIT -->|"creates PDA"| SYS
    CREATE -->|"creates PDA"| SYS

    style CREATE fill:#6366f1,stroke:#818cf8,color:#fff
    style DEPOSIT fill:#9945ff,stroke:#a78bfa,color:#fff
    style CHECKIN fill:#3b82f6,stroke:#60a5fa,color:#fff
    style REFUND fill:#22c55e,stroke:#4ade80,color:#000
    style CLAIM fill:#f59e0b,stroke:#eab308,color:#000
    style CLOSE fill:#ef4444,stroke:#f87171,color:#fff
    style SPL fill:#14f195,stroke:#22c55e,color:#000
```

---

## 13. Deployment & Infrastructure Topology

```mermaid
graph TB
    subgraph "Cloudflare Edge"
        direction TB
        CF["Cloudflare Workers<br/>(bethere worker)"]
        D1_INST["D1: bethere-db<br/>(SQLite, multi-region)"]
        KV_INST["KV: EVENTS namespace<br/>(event configs, cache)"]
        R2_INST["R2: bethere-assets<br/>(slip images, SVGs)"]
        CRON["Cron Trigger<br/>03:00 UTC daily<br/>(cleanup expired entries)"]
    end

    subgraph "Solana Cluster"
        MAINNET["Mainnet-Beta<br/>USDC: EPjFWdd5..."]
        DEVNET["Devnet<br/>USDC: 4zMMC9srt5..."]
        PROGRAM["bethere-escrow<br/>C6HDeZES9aP..."]
    end

    subgraph "Helius"
        RPC["RPC Endpoint<br/>(JSON-RPC)"]
        DAS["DAS API<br/>(getAssetsByOwner)"]
        MINT["mintCompressedNft<br/>(cNFT minting)"]
        WEBHOOK["Webhooks<br/>(enhanced TX)"]
    end

    subgraph "Google Cloud"
        GS["Google Sheets API<br/>(attendee registry)"]
        GA["Google OAuth 2.0<br/>(organizer auth)"]
    end

    subgraph "Browser"
        LEPTOS["Leptos WASM<br/>(CSR SPA)"]
        WALLET["Solana Wallet<br/>(Phantom, Solflare, Backpack)"]
    end

    CF --> D1_INST
    CF --> KV_INST
    CF --> R2_INST
    CRON --> CF
    CF --> RPC
    CF --> DAS
    CF --> MINT
    CF --> WEBHOOK
    WEBHOOK -->|"TX events"| CF
    CF --> GS
    CF --> GA
    CF -->|"serve static"| LEPTOS
    LEPTOS -->|"Solana Pay /<br/>wallet adapter"| WALLET
    WALLET -->|"sign TX"| PROGRAM
    PROGRAM --> MAINNET
    PROGRAM --> DEVNET

    style CF fill:#f59e0b,stroke:#eab308,color:#000
    style PROGRAM fill:#9945ff,stroke:#a78bfa,color:#fff
    style LEPTOS fill:#6366f1,stroke:#818cf8,color:#fff
```

---

## 14. API Surface — Worker Endpoints

| Category | Endpoint | Method | Auth | Description |
|---|---|---|---|---|
| **Event** | `/api/events` | GET | — | List public events |
| | `/api/events/{id}` | GET | — | Event detail |
| | `/api/events` | POST | Admin | Create event |
| | `/api/events/{id}` | PUT | Admin | Update event config |
| **Registration** | `/api/register` | POST | — | RSVP for event |
| | `/api/attendee/{id}` | GET | — | Attendee lookup |
| **Deposit (USDC)** | `/api/deposit/usdc` | POST | — | Build Solana Pay deposit TX |
| | `/api/deposit/status/{id}` | GET | — | Deposit status lookup |
| | `/api/deposit/usdc/refund` | POST | — | Build refund TX |
| | `/api/deposit/usdc/refund-and-close` | POST | — | Atomic refund + close |
| **Deposit (THB)** | `/api/deposit/thb/upload` | POST | — | Upload payment slip |
| | `/api/deposit/thb/pending` | GET | Admin | List pending slips |
| | `/api/deposit/thb/verify` | POST | Admin | Approve/reject slip |
| | `/api/refund/mark/{id}` | POST | Admin | Mark THB refund done |
| **Escrow** | `/api/escrow/init` | POST | Admin | Build init_escrow TX |
| | `/api/escrow/deactivate` | POST | Admin | Build deactivate TX |
| | `/api/escrow/close` | POST | Admin | Build close_event TX |
| | `/api/escrow/claim` | POST | Admin | Build claim_forfeited TX |
| | `/api/escrow/rollover` | POST | Admin | Build rollover_deposit TX |
| | `/api/escrow/index` | GET | Admin | On-chain event log |
| **Check-in** | `/api/checkin` | POST | Staff | QR code check-in |
| | `/api/walkin` | POST | Staff | Walk-in registration |
| **Claim** | `/api/claim/{token}` | GET | — | Claim status lookup |
| | `/api/claim/{token}` | POST | — | Mint cNFT to wallet |
| **Quiz** | `/api/quiz/questions` | GET | — | Get quiz questions |
| | `/api/quiz/submit` | POST | — | Submit quiz answers |
| **Adventure** | `/api/adventure/config` | GET | — | Get adventure config |
| | `/api/adventure/submit` | POST | — | Submit level answer |
| **Auth** | `/api/auth/login` | GET | — | Google OAuth redirect |
| | `/api/auth/callback` | GET | — | OAuth callback |
| | `/api/auth/me` | GET | JWT | Current user info |
| **Webhook** | `/api/webhook/escrow` | POST | Bearer | Helius escrow events |

---

## 15. Error Codes — On-Chain Program

| Code | Name | Description |
|---|---|---|
| 0 | `IncorrectDepositAmount` | Deposit ≠ escrow.deposit_amount |
| 1 | `RefundNotYetAllowed` | event_end not reached |
| 2 | `NotCheckedIn` | Attendee not checked in (legacy) |
| 3 | `RefundDeadlineNotPassed` | Organizer cannot claim yet |
| 4 | `AlreadyRefunded` | Double refund prevented |
| 5 | `AttendeeCheckedIn` | Checked-in deposit cannot be forfeited |
| 6 | `NoForfeitedFunds` | Nothing to claim |
| 7 | `EventNotActive` | Deposits rejected |
| 8 | `EventStillActive` | Cannot close active event |
| 9 | `Unauthorized` | Wrong signer |
| 10 | `VaultMismatch` | Wrong vault account |
| 11 | `MintMismatch` | Wrong USDC mint |
| 12 | `InvalidDepositAmount` | deposit_amount = 0 |
| 13 | `EventEndInPast` | event_end must be future |
| 14 | `Overflow` | Arithmetic overflow |
| 15 | `VaultNotEmpty` | Settle first |
| 16 | `EventEnded` | No check-ins after event |
| 17 | `DepositNotRefunded` | Cannot close unrefunded deposit |
| 18 | `EventEscrowStillActive` | Close deposits first |
| 19 | `RefundDeadlinePassed` | No-show forfeiture window |
| 20 | `EscrowVersionMismatch` | Unsupported schema |
| 21 | `DepositVersionMismatch` | Unsupported schema |
| 22 | `RefundRequiresClose` | refund must pair with close_deposit |

---

## Legend

| Symbol | Meaning |
|---|---|
| 🟣 Purple boxes | Solana on-chain components |
| 🟢 Green boxes | Token / value transfer |
| 🔵 Blue boxes | Off-chain infrastructure |
| 🟡 Yellow boxes | External services / trust-required |
| 🔴 Red boxes | Terminal / error states |
| → Solid arrows | Direct call / data flow |
| ⇢ Dashed arrows | Async / webhook notification |

---

## Evaluation Checklist

| Criterion | Status |
|---|---|
| All programs represented | ✅ bethere-escrow (9 instructions) |
| Account structures mapped | ✅ EventEscrow, AttendeeDeposit, Vault ATA |
| Program interactions illustrated | ✅ CPI to SPL Token, System, Associated Token |
| External dependencies shown | ✅ Helius, Google Sheets, OAuth, D1, KV, R2 |
| Decision points and alternate flows | ✅ In-Person vs Online vs Hybrid; USDC vs THB |
| Instruction ordering constraints | ✅ Lifecycle state diagram |
| Security model documented | ✅ Trustless vs trust-required |
| Clear, consistent labeling | ✅ Color-coded legend |
