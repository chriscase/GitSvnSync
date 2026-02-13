# GitSvnSync — Verification & Hardening Issue Plan

> **22 issues** across **6 phases** covering every module, script, and configuration file in the project.

## Quick Start

```bash
# Authenticate with GitHub CLI
gh auth login

# Create all labels and issues
bash .github/issues/create-all-issues.sh
```

---

## Execution Phases & Dependency Graph

```
PHASE 0: Foundation (must complete first)
  ┌──────────┐  ┌──────────┐  ┌──────────────┐
  │ 01 Compile│  │ 02 Tests │  │ 03 Web UI    │
  │ (Haiku)  │  │ (Sonnet) │  │ Build (Haiku)│
  │ LOCAL    │  │ LOCAL    │  │ LOCAL        │
  └────┬─────┘  └────┬─────┘  └──────┬───────┘
       │              │               │
       ▼              ▼               │
PHASE 1: Core Modules (all can run in parallel after Phase 0)
  ┌────────┐ ┌────────┐ ┌────────┐ ┌────────┐
  │04 SVN  │ │05 Git  │ │06 GitHub│ │07 DB   │
  │(Opus)  │ │(Opus)  │ │(Sonnet)│ │(Sonnet)│
  │LOCAL   │ │LOCAL   │ │CLOUD   │ │LOCAL   │
  └───┬────┘ └───┬────┘ └───┬────┘ └───┬────┘
      │          │          │          │
  ┌────────┐ ┌────────┐ ┌────────┐ ┌─────────────┐
  │08 Confli│ │09 Ident│ │10 Confi│ │11 Errors/   │
  │ct(Opus)│ │ty(Son.)│ │g(Son.) │ │Notify(Haiku)│
  │CLOUD   │ │CLOUD   │ │CLOUD   │ │CLOUD        │
  └───┬────┘ └───┬────┘ └───┬────┘ └─────┬───────┘
      │          │          │            │
      ▼          ▼          ▼            ▼
PHASE 2: Integration (after all Phase 1)
  ┌───────────────┐  ┌──────────┐  ┌──────────┐
  │12 Sync Engine │  │13 Daemon │  │14 CLI    │
  │(Opus) CRITICAL│  │(Sonnet)  │  │(Sonnet)  │
  │LOCAL          │  │LOCAL     │  │LOCAL     │
  └───────┬───────┘  └────┬─────┘  └──────────┘
          │               │
          ▼               ▼
PHASE 3: API & Web (after Phase 2)      PHASE 4: Infrastructure (after Phase 2)
  ┌─────────┐ ┌─────────┐ ┌────────┐     ┌──────────┐ ┌──────────┐
  │15 REST  │ │16 Webhks│ │17 WS   │     │19 CI/CD  │ │20 Docker │
  │API(Opus)│ │(Opus)   │ │(Sonnet)│     │(Sonnet)  │ │(Sonnet)  │
  │CLOUD    │ │CLOUD    │ │CLOUD   │     │CLOUD     │ │LOCAL     │
  └────┬────┘ └─────────┘ └───┬────┘     └──────────┘ └────┬─────┘
       │                      │                             │
       ▼                      ▼                             ▼
  ┌──────────────┐                                    ┌──────────┐
  │18 Web UI     │                                    │21 E2E    │
  │Verify(Sonnet)│                                    │(Sonnet)  │
  │CLOUD         │                                    │LOCAL     │
  └──────────────┘                                    └──────────┘
          │                                                │
          ▼                                                ▼
PHASE 5: Security (after ALL above)
  ┌───────────────────────────────┐
  │22 Full Security Audit         │
  │(Opus) — FINAL GATE            │
  │LOCAL                          │
  └───────────────────────────────┘
```

---

## Issue Summary Table

| # | Order | Title | Phase | Model | Agent | Priority | Depends On |
|---|-------|-------|-------|-------|-------|----------|------------|
| 1 | 01 | Verify Rust workspace compiles | 0 | Haiku | Local | Critical | — |
| 2 | 02 | Verify all 72 unit tests pass | 0 | Sonnet | Local | Critical | #1 |
| 3 | 03 | Verify web-ui builds cleanly | 0 | Haiku | Local | Critical | — |
| 4 | 04 | Verify SVN client & parser | 1 | Opus | Local | High | #1, #2 |
| 5 | 05 | Verify Git client wrapper | 1 | Opus | Local | High | #1, #2 |
| 6 | 06 | Verify GitHub API client | 1 | Sonnet | Cloud | High | #1, #2 |
| 7 | 07 | Verify database layer | 1 | Sonnet | Local | High | #1, #2 |
| 8 | 08 | Verify conflict engine | 1 | Opus | Cloud | High | #1, #2 |
| 9 | 09 | Verify identity mapper | 1 | Sonnet | Cloud | Medium | #1, #2 |
| 10 | 10 | Verify config system | 1 | Sonnet | Cloud | Medium | #1, #2 |
| 11 | 11 | Verify errors & notifications | 1 | Haiku | Cloud | Medium | #1, #2 |
| 12 | 12 | Verify sync engine | 2 | Opus | Local | **Critical** | #4-#11 |
| 13 | 13 | Verify daemon & scheduler | 2 | Sonnet | Local | High | #12, #10 |
| 14 | 14 | Verify CLI commands | 2 | Sonnet | Local | Medium | #1, #10, #7 |
| 15 | 15 | Verify REST API endpoints | 3 | Opus | Cloud | High | #1, #7, #8 |
| 16 | 16 | Verify webhook handlers | 3 | Opus | Cloud | High | #6, #12 |
| 17 | 17 | Verify WebSocket endpoint | 3 | Sonnet | Cloud | Medium | #1 |
| 18 | 18 | Verify web-ui React pages | 3 | Sonnet | Cloud | Medium | #3, #15, #17 |
| 19 | 19 | Verify CI/CD workflows | 4 | Sonnet | Cloud | Medium | #1, #2 |
| 20 | 20 | Verify Dockerfile | 4 | Sonnet | Local | Medium | #1 |
| 21 | 21 | Verify E2E test environment | 4 | Sonnet | Local | Medium | #20, #13 |
| 22 | 22 | Full security audit | 5 | Opus | Local | **Critical** | ALL |

---

## Parallel Execution Strategy

### Maximum Parallelism Plan

**Wave 1** (3 issues, parallel):
- `01` Compile check (LOCAL, Haiku)
- `03` Web UI build (LOCAL, Haiku)

**Wave 2** (1 issue, sequential):
- `02` Unit tests (LOCAL, Sonnet) — needs compile to pass first

**Wave 3** (8 issues, all parallel):
- `04` SVN client (LOCAL, Opus)
- `05` Git client (LOCAL, Opus)
- `06` GitHub API (CLOUD, Sonnet)
- `07` Database (LOCAL, Sonnet)
- `08` Conflict engine (CLOUD, Opus)
- `09` Identity mapper (CLOUD, Sonnet)
- `10` Config system (CLOUD, Sonnet)
- `11` Errors & notify (CLOUD, Haiku)

**Wave 4** (3 issues, parallel after Wave 3):
- `12` Sync engine (LOCAL, Opus)
- `14` CLI commands (LOCAL, Sonnet)
- `19` CI/CD workflows (CLOUD, Sonnet)

**Wave 5** (5 issues, parallel after Wave 4):
- `13` Daemon (LOCAL, Sonnet)
- `15` REST API (CLOUD, Opus)
- `16` Webhooks (CLOUD, Opus)
- `17` WebSocket (CLOUD, Sonnet)
- `20` Docker (LOCAL, Sonnet)

**Wave 6** (2 issues, parallel after Wave 5):
- `18` Web UI verify (CLOUD, Sonnet)
- `21` E2E tests (LOCAL, Sonnet)

**Wave 7** (1 issue, final):
- `22` Security audit (LOCAL, Opus) — after ALL others

---

## Agent Assignment

### Local Agents (12 issues)
Issues requiring filesystem access, CLI tools, Docker, or runtime testing:

| Issue | Why Local? |
|-------|-----------|
| 01 Compile | Needs Rust toolchain |
| 02 Tests | Needs Rust + SVN CLI |
| 03 Web UI Build | Needs Node.js + npm |
| 04 SVN Client | Needs SVN CLI for integration tests |
| 05 Git Client | Needs git for test repo creation |
| 07 Database | Needs SQLite for real DB tests |
| 12 Sync Engine | Complex call chain tracing |
| 13 Daemon | Signal handling, process lifecycle |
| 14 CLI | Binary execution testing |
| 20 Docker | Needs Docker daemon |
| 21 E2E | Needs Docker Compose + all services |
| 22 Security | Needs cargo audit + npm audit + runtime testing |

### Cloud Agents / GitHub Copilot (10 issues)
Pure code review that doesn't need local execution:

| Issue | Why Cloud? |
|-------|-----------|
| 06 GitHub API | HTTP API code review |
| 08 Conflict Engine | Pure algorithm verification |
| 09 Identity Mapper | Logic review, no external deps |
| 10 Config System | Parsing/validation review |
| 11 Errors & Notify | Type hierarchy review |
| 15 REST API | Endpoint security review |
| 16 Webhooks | Webhook handler review |
| 17 WebSocket | Connection lifecycle review |
| 18 Web UI Verify | React component review |
| 19 CI/CD | YAML workflow review |

---

## Model Recommendations

### Claude Haiku (3 issues) — Fast, Low-Cost
Best for: straightforward verification, compilation checks, simple code review
- `01` Compile verification
- `03` Web UI build
- `11` Errors & notifications

### Claude Sonnet (12 issues) — Balanced
Best for: moderate analysis, test verification, API review, infrastructure
- `02` Unit tests
- `06` GitHub API client
- `07` Database layer
- `09` Identity mapper
- `10` Config system
- `13` Daemon
- `14` CLI commands
- `17` WebSocket
- `18` Web UI verify
- `19` CI/CD workflows
- `20` Docker
- `21` E2E tests

### Claude Opus (7 issues) — Deep Analysis
Best for: security-critical code, complex algorithms, state machines, attack surface analysis
- `04` SVN client (command injection, XML parsing security)
- `05` Git client (data loss prevention, credential security)
- `08` Conflict engine (algorithmic correctness)
- `12` Sync engine (state machine soundness, crash recovery)
- `15` REST API (auth bypass, input validation)
- `16` Webhooks (signature verification, replay attacks)
- `22` Security audit (comprehensive OWASP review)

---

## Key Rule: When Problems Are Found

**Every issue includes this mandate:**

> When problems are found during verification:
> 1. **Fix directly** if the fix is straightforward (typos, missing validation, simple bugs)
> 2. **Create a new issue** if the fix is non-trivial, titled:
>    - `[Fix] <module>: <description>` for bugs
>    - `[Fix][Security] <module>: <description>` for security issues
>    - `[Fix][Critical] <module>: <description>` for data loss / crash risks
>    - `[Test Gap] <module>: <description>` for missing test coverage
> 3. New issues must:
>    - Reference the parent verification issue
>    - Include the exact error/file/line
>    - Be labeled with `type:fix-required` or `type:test-gap`
>    - Include recommended model and agent assignment

---

## Labels Reference

| Label | Color | Purpose |
|-------|-------|---------|
| `phase:0-foundation` | Green | Build & compile verification |
| `phase:1-core` | Blue | Core module verification |
| `phase:2-integration` | Purple | Cross-module verification |
| `phase:3-api-web` | Orange | API, web, UI verification |
| `phase:4-infra` | Pink | CI/CD, Docker, deployment |
| `phase:5-security` | Red | Security audit |
| `model:claude-haiku` | Light Blue | Fast, straightforward tasks |
| `model:claude-sonnet` | Light Teal | Moderate complexity |
| `model:claude-opus` | Lavender | Deep analysis |
| `agent:local` | Yellow | Needs local filesystem/tools |
| `agent:cloud` | Blue | Remote/Copilot agent |
| `agent:either` | Teal | Flexible |
| `type:verification` | Olive | Verification task |
| `type:fix-required` | Red | Problem found, needs fix |
| `type:test-gap` | Cream | Missing test coverage |
| `priority:critical` | Red | Blocks other work |
| `priority:high` | Orange | Do soon |
| `priority:medium` | Yellow | Standard |
| `priority:low` | Green | Can defer |
| `order:01`–`order:22` | Gray | Execution sequence |
