# gh-status: GitHub PR Dashboard for macOS

## Context

**Problem**: Managing 20+ active GitHub repos with stacked PRs across 3-5 orgs and 2 accounts (personal + work) means context is scattered. There's no single tool that combines multi-repo PR aggregation, stack visualization, and native desktop presence.

**Existing tools fall short**:
- **Graphite**: Best stacked PRs tool, but SaaS-dependent, requires their workflow, single-account only
- **GitKraken/Tower**: Great git GUIs, but no multi-repo dashboard, no stack visualization, heavy Electron
- **Gitify/Trailer**: macOS notification aggregators, but notification-only -- no PR details, review, or stacks
- **Octobox**: Web notification dashboard, but no code review, no stacks, not native
- **gh CLI / git-branchless**: CLI-only, single-repo, no dashboard

**Our gap**: Multi-repo PR dashboard + stack tree visualization + native macOS app + multi-account + read-only review, all in one lightweight tool.

---

## Tech Stack

| Layer | Choice | Rationale |
|-------|--------|-----------|
| Desktop framework | **Tauri 2.0** | Rust backend, ~50MB vs Electron's ~150MB, native macOS integration |
| Backend | **Rust** (octocrab, tokio, serde, keyring) | Type safety, performance, macOS Keychain via keyring crate |
| Frontend | **React 18 + TypeScript + Vite** | Mature ecosystem, fast dev iteration |
| State | **Zustand** (UI) + **TanStack Query** (server) | Clean separation of UI state vs cached server data |
| UI kit | **shadcn/ui + Tailwind CSS** | Headless components, dark/light mode, highly customizable |
| GitHub API | **GraphQL v4** primary (batched), REST v3 for future webhooks | Single query fetches 20 repos vs 60+ REST calls |
| Auth storage | **macOS Keychain** (keyring crate) | Never store tokens in files |
| Testing | **Rust #[test]** + **Vitest** + **Playwright** + **Docker mock server** | Full coverage: unit, integration, E2E |
| CI | **GitHub Actions** | Lint + test + build on every push |

**API rate limits are not a concern**: ~20 repos polled every 2 min with GraphQL batching = ~60-120 points/hour out of 5,000/hour limit per account.

---

## Architecture

```
┌─────────────────────────────────────────────────────────┐
│                    macOS Desktop                         │
│  ┌────────────┐  ┌──────────────────────────────────┐   │
│  │ System Tray │  │         Main Window               │   │
│  │ (badge +    │  │  React 18 + TypeScript            │   │
│  │  dropdown)  │  │  Zustand + TanStack Query         │   │
│  └──────┬──────┘  │  shadcn/ui + Tailwind             │   │
│         │         └──────────────┬───────────────────┘   │
│         │    invoke() / events   │                        │
│         ▼                        ▼                        │
│  ┌────────────────────────────────────────────────────┐  │
│  │           Rust Backend (Tauri Commands)              │  │
│  │                                                      │  │
│  │  accounts  │  github   │  sync     │  stacks        │  │
│  │  (keyring, │  (graphql,│  (poll,   │  (detect,      │  │
│  │   multi-   │   client, │   diff,   │   tree,        │  │
│  │   account) │   types)  │   events) │   ordering)    │  │
│  │            │           │           │                 │  │
│  │  store     │  tray     │  commands │  notifications │  │
│  │  (persist, │  (icon,   │  (IPC     │  (macOS        │  │
│  │   prefs)   │   badge)  │   layer)  │   native)      │  │
│  └────────────────────────┬───────────────────────────┘  │
│                           │                                │
│                    macOS Keychain (PATs)                   │
└───────────────────────────┼──────────────────────────────┘
                            │ HTTPS
                  ┌─────────▼─────────┐
                  │ GitHub GraphQL v4  │
                  │ (per-account)      │
                  └───────────────────┘
```

### Data Flow

```
Polling Loop (every 2 min per account)
  ├─► Batched GraphQL query (PRs + reviews + comments + branch info)
  │     ├─► Diff against in-memory state
  │     │     ├─► New comments/approvals/CI → macOS notification
  │     │     ├─► State update → Tauri event → React re-render
  │     │     └─► Badge count update → tray icon refresh
  │     └─► Stack detection (base branch chain analysis)
  └─► Persist state to Tauri store (crash recovery)

User Action (click PR)
  ├─► React → invoke("get_pr_details") → Rust returns cached data instantly
  └─► Background refresh if stale (>30s)
```

### Key Design Decisions

| Decision | Rationale |
|----------|-----------|
| Tauri store (JSON) over SQLite | 20-40 PRs = <1MB in memory. SQLite adds migration complexity for no benefit at this scale. |
| GraphQL over REST | 2-3 batched queries vs 60-80 REST calls for 20 repos |
| Polling over webhooks | Desktop app has no public endpoint. 2-min poll uses ~720 points/day, well within limits. |
| Zustand + TanStack Query over Redux | UI state and server cache have fundamentally different semantics. No reason to conflate them. |
| Cache-first + event invalidation | Commands return cached data instantly. Rust sync emits events → TanStack Query invalidates → re-fetches from updated cache. No loading spinners on routine updates. |

---

## Roadmap (6 Projects)

| # | Project | Goal | Key Deliverables | Timeline | Status |
|---|---------|------|-------------------|----------|--------|
| 1 | **Foundation** | Auth + fetch + display PRs | Tauri scaffold, multi-account auth, GraphQL client, PR list, sync engine, CI | Weeks 1-4 | **CURRENT** |
| 2 | **Dashboard + Stacks** | Daily-driver morning experience | Morning dashboard, stack detection + tree viz, tray + notifications, filtering | Weeks 5-9 | |
| 3 | **PR Detail + Review** | Read-only code review in-app | PR detail view, timeline, diff viewer (syntax highlighted), keyboard nav | Weeks 10-14 | |
| 4 | **Claude + Multi-Account** | Power-user integrations | Claude context copier, comment from any account, quick-action browser links | Weeks 15-18 | |
| 5 | **Polish + Performance** | Production UX quality | Window persistence, offline resilience, adaptive rate limits, search, shortcuts, auto-updater | Weeks 19-22 | |
| 6 | **Beta + Distribution** | Ship it | Code signing, notarization, auto-update, onboarding wizard, crash reporting, docs | Weeks 23-26 | |

---

## Project 1: Foundation — Detailed

**Goal**: Buildable Tauri app that authenticates with GitHub, fetches PRs, and displays them in a usable list.

### Milestones

| Milestone | Description | Days | Status |
|-----------|-------------|------|--------|
| M1.1 Scaffold | Tauri 2.0 + React/Vite project, Rust module skeleton, CI green | 3 | **CURRENT** |
| M1.2 Auth | Multi-account PAT via Keychain, settings UI, token validation | 4 | |
| M1.3 GitHub Client | GraphQL batched queries, PR parsing, domain types | 5 | |
| M1.4 Sync Engine | Polling loop, state diffing, Tauri event emission | 4 | |
| M1.5 PR List UI | Grouped/flat list, filters, status indicators, empty states | 4 | |

### Scope

**In**: Auth, PR fetching (title, status, author, reviews, CI), list UI, polling, CI, tests
**Out**: Tray icon, notifications, stacks, diff view, dashboard, keyboard shortcuts

---

### Phase 1: Scaffold and Tooling (Days 1-3)

**Goal**: Running Tauri app with full project structure, CI green.

**Step 1.1 — Initialize Tauri project**

```
gh-status/
├── package.json
├── vite.config.ts
├── tsconfig.json
├── tailwind.config.ts
├── index.html
├── src/                          # React frontend
│   ├── main.tsx
│   ├── App.tsx
│   └── globals.css
├── src-tauri/                    # Rust backend
│   ├── Cargo.toml
│   ├── build.rs
│   ├── tauri.conf.json
│   ├── capabilities/default.json
│   └── src/
│       ├── main.rs               # calls lib::run()
│       └── lib.rs                # app builder
├── docker/                       # Mock servers for testing
│   └── docker-compose.yml
├── tests/                        # E2E tests
│   └── e2e/
└── .github/workflows/ci.yml
```

Validation: `cargo tauri dev` launches window with React rendering.

**Step 1.2 — Rust module skeleton**

```
src-tauri/src/
├── lib.rs                        # mod declarations, app builder
├── error.rs                      # AppError (thiserror + serde::Serialize)
├── commands/
│   ├── mod.rs                    # re-exports
│   ├── accounts.rs
│   ├── pulls.rs
│   └── sync.rs
├── github/
│   ├── mod.rs
│   ├── client.rs                 # GitHubClient wrapping octocrab
│   ├── graphql.rs                # query builder + response parsing
│   └── types.rs                  # API DTOs
├── accounts/
│   ├── mod.rs                    # AccountManager
│   └── keyring.rs                # Keychain storage
├── sync/
│   ├── mod.rs                    # SyncEngine
│   └── differ.rs                 # state diff logic
├── store/
│   └── mod.rs                    # persistent cache + prefs
└── types/
    └── mod.rs                    # domain types (PullRequest, Review, etc.)
```

Design rule: `commands/` is a thin adapter layer. Business logic lives in domain modules.

Validation: `cargo check` + `cargo clippy -- -D warnings` pass.

**Step 1.3 — Frontend skeleton**

```
src/
├── lib/
│   ├── utils.ts                  # cn() helper
│   └── tauri.ts                  # typed invoke() wrappers
├── hooks/
│   ├── use-tauri-event.ts        # Rust event → React
│   └── use-pull-requests.ts      # TanStack Query hook
├── stores/
│   ├── ui-store.ts               # sidebar, view, filters
│   └── account-store.ts
├── components/
│   ├── ui/                       # shadcn primitives
│   ├── layout/
│   │   ├── Sidebar.tsx
│   │   ├── MainPanel.tsx
│   │   └── StatusBar.tsx
│   └── pr/
│       └── PRListItem.tsx
├── views/
│   ├── PRListView.tsx
│   └── SettingsView.tsx
└── types/
    └── index.ts                  # mirrors Rust types (camelCase)
```

Validation: Layout shell renders with sidebar + main panel. Dark mode follows system.

**Step 1.4 — CI pipeline**

`.github/workflows/ci.yml` with 3 jobs:
1. `rust-check`: fmt, clippy, test
2. `frontend-check`: eslint, tsc --noEmit, vitest
3. `tauri-build`: `cargo tauri build --debug` (macos-latest)

Validation: Push to GitHub, all jobs green.

**Exit criteria**: App renders, all modules exist, CI passes, clippy clean.

---

### Phase 2: Authentication (Days 4-7)

**Goal**: Add/remove GitHub accounts, PATs in Keychain, validate against GitHub API.

**Auth evolution**: MVP uses PAT + Keychain (simple, proven). Project 2 adds OAuth Device Flow (same flow as `gh auth login` — user gets a code, opens browser, authorizes, app receives scoped token). Keychain storage stays the same regardless of token type.

**Step 2.1 — Keychain integration** (`accounts/keyring.rs`)

```rust
pub struct KeychainStore { service_name: String }
// Methods: store_token, get_token, delete_token, list_accounts
// Uses keyring crate with apple-native feature
// Service name: "com.gh-status.github-pat"
```

**Step 2.2 — Account manager** (`accounts/mod.rs`)

```rust
pub struct Account { id, username, avatar_url, orgs }
pub struct AccountManager { keychain, accounts: RwLock<Vec<Account>> }
// add_account validates token via GitHub viewer query, fetches profile
// Tokens never held in memory — fetched from Keychain on demand
```

**Step 2.3 — Account commands** (`commands/accounts.rs`)

`add_account`, `remove_account`, `list_accounts` — thin wrappers delegating to AccountManager.

**Step 2.4 — Settings UI** (`views/SettingsView.tsx`)

Form: label + PAT input → validates → shows username + avatar. Account list with remove buttons.

**Exit criteria**: Account persists across restarts. Token in Keychain only. Invalid tokens show clear error. Unit tests pass.

---

### Phase 3: GitHub Client + PR Fetching (Days 8-12)

**Goal**: Batched GraphQL queries fetch all PRs across all repos for all accounts.

**Step 3.1 — GraphQL query builder** (`github/graphql.rs`)

Batched using aliases — single query fetches PRs for up to 10 repos:
```graphql
query { repo_0: repository(owner:"org", name:"repo") { pullRequests(...) { ... } } ... }
```
Repos chunked into groups of 10 to stay under complexity limits. Parse via `serde_json::Value` walking aliased keys.

**Step 3.2 — GitHub client** (`github/client.rs`)

```rust
pub struct GitHubClient { octocrab, account_id }
// fetch_prs(repos) → batched GraphQL, returns Vec<PullRequest>
// fetch_pr_detail(owner, repo, number) → full PR with comments
// verify_token() → viewer info
// Tracks X-RateLimit-Remaining headers
```

**Step 3.3 — Domain types** (`types/mod.rs`)

```rust
pub struct PullRequest {
    id, number, title, state, is_draft, author, repo: RepoRef,
    account_id, base_ref, head_ref, url, created_at, updated_at,
    mergeable, review_status, review_requests, ci_status,
    comment_count, labels, last_comment_at
}
```

TypeScript mirrors with `camelCase` via `#[serde(rename_all = "camelCase")]`.

**Step 3.4 — Repo configuration** (`store/mod.rs`)

Manual repo watchlist for MVP (owner/repo per account). Auto-discovery deferred to Project 2.

**Step 3.5 — Docker mock server** (`docker/`)

Lightweight Express/Fastify server returning canned GraphQL responses. `docker-compose.yml` for integration tests.

**Exit criteria**: Batched GraphQL fetches PRs for all repos. Types round-trip Rust ↔ TS cleanly. Docker mock tests pass.

---

### Phase 4: Sync Engine (Days 13-16)

**Goal**: Background polling fetches PRs every 2 min, detects changes, pushes events to frontend.

**Step 4.1 — SyncEngine** (`sync/mod.rs`)

```rust
pub struct SyncEngine { state: Arc<RwLock<SyncState>>, app_handle }
// start() — spawns tokio task per account
// sync_now() — immediate refresh
// get_prs(filter) — read from cache
```

Per-account polling: sleep → fetch → diff → emit events → update state → persist.

**Step 4.2 — Change diffing** (`sync/differ.rs`)

```rust
pub enum ChangeKind { NewComments(u32), ReviewStatusChanged, CiStatusChanged,
    TitleChanged, Merged, Closed, DraftStatusChanged, NewReviewRequest }
pub fn diff_pr_states(old, new) -> Vec<PrChange>
```

**Step 4.3 — Tauri event bridge**

Rust emits `prs-updated` and `sync-status` events. React hook `useTauriEvent` listens and invalidates TanStack Query cache, triggering re-fetch from updated Rust cache.

**Step 4.4 — Manual sync command + status bar**

Refresh button in UI. Status bar shows "Last synced: 2m ago" and spinner during sync.

**Exit criteria**: Auto-refresh works. Changes detected. Frontend updates reactively. Manual sync works. Differ has 10+ edge case tests.

---

### Phase 5: PR List UI (Days 17-20)

**Goal**: Usable, well-designed PR list with grouping, indicators, and filtering.

**Step 5.1 — PR list item**

Compact row: avatar, title, `owner/repo`, `#number`, review status icon, CI status icon, comment count, relative time, draft badge, label dots.

**Step 5.2 — Grouped + flat views**

Grouped by repo (collapsible) or flat (sorted by updated_at). Toggle button in UI.

**Step 5.3 — Filter bar**

Account pills, search input, draft toggle, review status dropdown. Filters applied server-side in Rust command for performance.

**Step 5.4 — Empty states**

No accounts → onboarding card. No repos → "Add repos" prompt. Syncing → skeleton. No matches → clear-filters button. API error → retry banner.

**Step 5.5 — Test suite**

- Vitest: filter logic, date formatting, status derivation
- React Testing Library: PRListItem, filter bar components
- Playwright E2E: add account → add repo → see PR list (against Docker mock)
- Rust: integration tests for all command handlers

**Exit criteria**: App is usable as a daily PR viewer. Both list modes work. Filters work. Status indicators correct. Playwright E2E passes. CI green.

---

## Project 2: Dashboard + Stacks — Detailed

**Goal**: The core daily-driver experience — open the app and see what needs attention, with stack visualization.

### Milestones

| Milestone | Description | Days |
|-----------|-------------|------|
| M2.1 Dashboard Data | Rust-side PR categorization + aggregation commands | 4 |
| M2.2 Dashboard UI | React dashboard: needs attention, waiting, recent activity | 4 |
| M2.3 Stack Detection | Base branch chain analysis, tree construction, cycle handling | 5 |
| M2.4 Stack Tree UI | Vertical tree sidebar, expand/collapse, status indicators | 4 |
| M2.5 Tray + Notifications | System tray icon + badge, macOS native notifications, prefs | 5 |
| M2.6 OAuth Device Flow | Replace PAT entry with `gh auth login`-style browser auth flow | 3 |

### Stack Detection Algorithm

```
For each repo's open PRs:
  For each PR:
    if PR.base_ref == another_PR.head_ref (same repo):
      PR is child of that other PR

  Build: parent_map[pr_id] = parent_pr_id
  Roots: PRs where base_ref is main/master/develop
  Tree: DFS from roots → ordered stack chains
  Cycles: break at oldest PR (shouldn't happen in practice)
```

O(n^2) per repo but n ≤ ~10 PRs per repo → effectively instant.

### Dashboard Sections

1. **Needs Your Attention**: PRs where you're requested reviewer, or have new comments addressed to you
2. **Waiting on Review**: Your PRs awaiting review from others
3. **Recent Activity**: Timeline of last 24h — new comments, approvals, CI changes
4. **Stale Check**: Your PRs with no activity in 3+ days

### Stack Tree Visualization (sidebar)

```
main
 +-- feat/auth (#101) [approved]
 |   +-- feat/auth-oauth (#102) [changes requested]
 |       +-- feat/auth-refresh (#103) [draft]
 +-- feat/payments (#104) [1/2 approved]
     +-- feat/payments-stripe (#105) [CI failing]
feat/standalone-fix (#106) [approved]
```

Each node shows: PR number, short title, review status icon, CI status icon. Click to select. Expand/collapse stacks.

### Scope

**In**: Dashboard sections, stack detection + tree, tray icon + badge, macOS notifications, filter by account/repo/stack
**Out**: Diff view, code review, Claude integration, commenting, search

---

## Verification Plan

### Per-Phase Testing

| Layer | Tool | What |
|-------|------|------|
| Rust unit | `cargo test` | Domain logic, differ, stack detection, GraphQL parsing |
| Rust integration | `cargo test` + Docker mock | Full command handler flows against mock API |
| Frontend unit | Vitest | Filter logic, formatters, store behavior |
| Component | React Testing Library | PR list items, filter bar, dashboard sections |
| E2E | Playwright + Docker | Full flows: add account → see PRs → filter → stack view |

### End-to-End Smoke Test (Project 1)

1. `docker-compose up` (starts GitHub mock server)
2. Launch app with `cargo tauri dev`
3. Add account (uses mock token)
4. Add repo to watchlist
5. See PR list populate
6. Wait 2 min → verify auto-refresh
7. Filter by review status → verify list updates
8. Remove account → verify cleanup

### End-to-End Smoke Test (Project 2)

1. Same setup as Project 1
2. Verify dashboard shows categorized PRs
3. Verify stack tree shows correct parent-child relationships
4. Verify tray icon shows unread badge count
5. Trigger a mock "new comment" → verify macOS notification fires
6. Click notification → verify app opens to relevant PR
