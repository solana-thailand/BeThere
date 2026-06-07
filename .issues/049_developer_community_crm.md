# Issue 049: Developer Community CRM

## Summary

Build a developer profile system and community CRM that collects, aggregates, and leverages developer data across events. Enables organizers (Solana Thailand, SuperteamTH) to understand their community, plan targeted events, and run multi-event campaigns (bootcamps, hackathons, workshop series).

## Motivation

Currently the platform collects transactional data (name, email, check-in status) but not **profile data** (skills, interests, experience level, learning goals). This limits:

- **Community insight**: Organizers can't answer "how many Rust developers in Bangkok?"
- **Event planning**: No data on what topics the community wants
- **Campaign tracking**: No way to manage multi-event series (bootcamp → hackathon → demo day)
- **Talent pipeline**: No way to identify developers who completed a learning track
- **Retention**: No personalized follow-up based on interests

## Architecture Overview

### New D1 Tables

1. **`developer_profiles`** — Rich developer profile, built incrementally across events
2. **`registration_responses`** — Per-event configurable form responses feeding into profiles
3. **`campaigns`** — Event series (bootcamp, hackathon track)
4. **`campaign_events`** — Ordered events within a campaign
5. **`developer_campaign_progress`** — Where each developer is in a campaign

### Data Flow

```
Registration Form → registration_responses (raw answers)
                      ↓ (if profile_field)
                  developer_profiles (accumulated profile)
                      ↓
                  Community Dashboard (organizer insights)
                      ↓
                  Campaign matching (targeted invitations)
```

### Profile Building Strategy

- Registration forms are **configurable per event** (stored in KV as JSON)
- Fields marked `profile_field: true` upsert into `developer_profiles`
- Profile keeps the **latest** answer for each field (overwritten on next event)
- `first_seen_at` / `total_events` auto-maintained by the system

## Phases

### Phase 1: D1 Migration + Developer Profiles (This Issue)
- [x] D1 Phase 2a: Dual-write for attendees, contacts, events, staff (Issue #046)
- [x] `developer_profiles` table + upsert query
- [x] `registration_responses` table + insert query
- [x] Backend: registration handler writes to D1 + developer profile (experience_level, tech_stack, interests, consent_outreach)
- [x] Backend: community insights API (aggregations)
- [x] Frontend: developer profile fields on registration form
- [x] Backend: developer list API (paginated)
- [x] Registration form config schema (KV-stored JSON per event)

### Phase 2: Configurable Registration Forms
- [x] Admin UI: form field builder (drag-drop, field types)
- [x] Public event page: dynamic form rendering from config
- [x] Profile enrichment: each registration updates developer profile

### Phase 3: Campaigns & Series
- [ ] `campaigns` + `campaign_events` tables
- [ ] `developer_campaign_progress` tracking
- [ ] Campaign dashboard: completion rates, drop-off points
- [ ] Completion certificate NFT for series graduates

### Phase 4: Organizer Community Dashboard
- [ ] Skills distribution chart
- [ ] Interest heat map
- [ ] Event attendance funnel (registered → checked in → completed)
- [ ] Developer search/filter for targeted outreach
- [ ] Campaign ROI metrics

## Database Schema

### developer_profiles

| Column | Type | Notes |
|--------|------|-------|
| email (PK) | TEXT | Lowercased, from OAuth login |
| display_name | TEXT | From registration |
| wallet_address | TEXT | Solana wallet (nullable) |
| github_handle | TEXT | |
| discord_handle | TEXT | |
| twitter_handle | TEXT | |
| experience_level | TEXT | beginner/mid/senior/lead |
| primary_role | TEXT | dev/designer/pm/founder/student |
| tech_stack | TEXT | JSON array: ["Rust","TypeScript"] |
| interests | TEXT | JSON array: ["DeFi","ZK","Gaming"] |
| learning_goals | TEXT | Free text |
| company_org | TEXT | Current employer/org |
| location_city | TEXT | |
| consent_outreach | INTEGER | PDPA: can we contact? 0/1 |
| first_seen_at | TEXT | Auto: first registration |
| last_active_at | TEXT | Auto: last event interaction |
| total_events | INTEGER | Auto: denormalized count |
| badges_earned | TEXT | JSON array of badge IDs |
| created_at | TEXT | |
| updated_at | TEXT | |

### registration_responses

| Column | Type | Notes |
|--------|------|-------|
| id (PK) | TEXT | UUID v7 |
| event_id | TEXT | Which event |
| developer_email | TEXT | Who |
| field_key | TEXT | e.g. "experience_level" |
| field_value | TEXT | JSON-serializable |
| is_profile_field | INTEGER | 1 = update developer_profiles |
| answered_at | TEXT | |

### campaigns

| Column | Type | Notes |
|--------|------|-------|
| id (PK) | TEXT | slug |
| title | TEXT | |
| description | TEXT | |
| organization_id | TEXT | |
| status | TEXT | draft/active/completed |
| completion_criteria | TEXT | JSON config |
| reward_type | TEXT | nft_certificate/badge |
| reward_config | TEXT | JSON metadata |
| created_at | TEXT | |
| updated_at | TEXT | |

### campaign_events

| Column | Type | Notes |
|--------|------|-------|
| campaign_id | TEXT | |
| event_id | TEXT | |
| sequence_order | INTEGER | |
| is_required | INTEGER | 1 = must attend |
| PRIMARY KEY | (campaign_id, event_id) | |

### developer_campaign_progress

| Column | Type | Notes |
|--------|------|-------|
| campaign_id | TEXT | |
| developer_email | TEXT | |
| events_completed | INTEGER | |
| total_required | INTEGER | |
| is_complete | INTEGER | |
| completed_at | TEXT | |
| reward_claimed_at | TEXT | |
| PRIMARY KEY | (campaign_id, developer_email) | |

## Registration Form Config (per event, stored in KV)

```json
{
  "fields": [
    {
      "key": "experience_level",
      "label": "What is your development experience level?",
      "type": "select",
      "options": ["Beginner", "Intermediate", "Senior", "Tech Lead"],
      "required": true,
      "profile_field": true
    },
    {
      "key": "tech_stack",
      "label": "Which technologies do you use?",
      "type": "multiselect",
      "options": ["Rust", "TypeScript", "Python", "Solidity", "Move", "Go", "C++"],
      "required": true,
      "profile_field": true
    },
    {
      "key": "interests",
      "label": "What topics interest you?",
      "type": "multiselect",
      "options": ["DeFi", "NFT", "ZK Proofs", "Infrastructure", "Gaming", "AI/ML", "Mobile"],
      "required": false,
      "profile_field": true
    },
    {
      "key": "learning_goals",
      "label": "What do you hope to learn?",
      "type": "textarea",
      "required": false,
      "profile_field": true
    },
    {
      "key": "dietary_restrictions",
      "label": "Dietary restrictions (for catering)",
      "type": "text",
      "required": false,
      "profile_field": false
    }
  ]
}
```

## Dependencies

- Issue #046 Phase 2a (D1 dual-write) — **in progress**
- Issue #037 Phase 1 (D1 claim locks + audit) — ✅ COMPLETE

## Status

- [x] Phase 1: D1 Migration + Developer Profiles (complete)
- [x] Phase 2: Configurable Registration Forms (complete)
- [ ] Phase 3: Campaigns & Series
- [ ] Phase 4: Organizer Community Dashboard
