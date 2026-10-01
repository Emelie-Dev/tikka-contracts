# Oracle Service Architecture

This document explains the design of the Tikka randomness oracle service, which implements the off-chain component of External and Quorum randomness modes. For operational runbooks and alerting, see [oracle/RUNBOOK.md](../oracle/RUNBOOK.md).

## Overview

The oracle service is a Node.js application that polls the Soroban network for `RandomnessRequested` events, generates randomness seeds, and submits them on-chain via `provide_randomness` transactions. It is **required** for `RandomnessSource::External` and `RandomnessSource::Quorum` raffle modes.

### Key Responsibilities

- Poll Soroban RPC for `RandomnessRequested` events
- Detect and skip duplicate requests (crash-safe via persistent dedup store)
- Generate VRF randomness proofs or cryptographic seeds
- Submit `provide_randomness` transactions with automatic retry and backoff
- Track ledger checkpoints to enable restart recovery
- Emit operational alerts for queue depth, RPC failures, and submission errors
- Expose `/health` and `/metrics` endpoints for Prometheus monitoring

## Pipeline Architecture

The oracle service is structured as a pipeline with clear separation of concerns:

```
EventListener → RequestQueue → VrfService → TxSubmitter → Checkpoint
     ↓              ↓              ↓             ↓             ↓
  polls RPC   buffers jobs   signs proofs   submits TXs   persists state
  discovers   tracks age      derives seed   retry logic   ledger number
  events      monitors depth  binds context  rate limits   crash recovery
```

### 1. Event Listener (KeyService + EventListenerService)

The event listener polls Soroban RPC at a configurable interval (default: 5 seconds) for `RandomnessRequested` events.

**Responsibilities:**
- Load the oracle's Ed25519 keypair securely from environment or secrets manager
- Poll the factory contract for new events since the last observed ledger
- Validate that the event is addressed to this oracle (for single-oracle mode) or broadcast to all participating oracles (for quorum mode)
- Emit event correlation fields (`requestId`, `raffleContract`) for request-scoped logging
- Persist the last processed ledger number to `./data/checkpoint.json`

**Crash Safety:**
If the service crashes during polling, the checkpoint on restart will resume from the last persisted ledger. If the checkpoint is older than the RPC retention window (typically 128 ledgers), events may be missed and must be recovered manually.

### 2. Request Queue (RequestQueue)

The request queue is an in-memory buffer with health monitoring.

**Responsibilities:**
- Accept enqueued jobs with FIFO ordering
- Track queue depth and age of the oldest pending job
- Expose metrics: current queue depth, age of oldest job
- Enable graceful shutdown by draining all pending jobs before exit

**Data Structure:** Jobs are held entirely in Node.js process memory. They are NOT persisted to disk.

**Crash Safety:** If the service crashes with pending jobs in the queue, those jobs are **lost**. Recovery depends on the oracle timeout window on-chain:
- If the raffle's `RandomnessRequested` event is re-emitted within `ORACLE_TIMEOUT_LEDGERS` (contract parameter), the oracle will pick it up again after restart.
- If the timeout expires before restart, the raffle falls back to internal PRNG seed derivation on-chain.

### 3. VRF Service (VrfService)

The VRF service generates randomness seeds bound to each specific raffle and request.

**Single-Oracle Mode:**
The oracle signs a message containing the raffle contract address and request ID, producing an Ed25519 signature. The first 8 bytes of SHA-256(signature) are extracted and interpreted as a big-endian u64 seed. The on-chain verifier independently derives the same value from the signature proof.

**Quorum Mode:**
Each oracle in the quorum generates its seed from 8 random bytes returned by Node.js `crypto.randomBytes()`. These seeds are concatenated on-chain, and the draw uses SHA-256(seed_1 || seed_2 || ... || seed_n) as the final source.

**Randomness Sources:**
- Single-oracle: Deterministic signature-based proof
- Quorum mode: Non-deterministic cryptographic randomness per oracle

### 4. Transaction Submitter (TxSubmitterService)

The submitter broadcasts `provide_randomness` transactions to Soroban RPC with automatic retry logic.

**Responsibilities:**
- Build a signed transaction for `provide_randomness` with the seed and proof
- Submit via RPC (`sendTransaction`)
- Detect transient failures (queue full, timeout, fee too low) and retry with exponential backoff
- Track submission outcome (success, retry, fatal)
- Emit metrics: submission count by outcome, cumulative fees spent

**Retry Logic:**
- Base backoff: `ORACLE_RETRY_BASE_MS` (default: 500ms)
- Max backoff: `ORACLE_RETRY_MAX_MS` (default: 30 seconds)
- Max attempts: `ORACLE_RETRY_MAX_ATTEMPTS` (default: 5)

**Failure Modes:**
- **Transient (retry):** Network timeout, RPC queue full, temporarily insufficient balance
- **Fatal (dead-letter):** Invalid request ID, wrong raffle, insufficient key material

Failed jobs are tracked in dead-letter metrics and can trigger alerts after a threshold.

### 5. Deduplication Store (DeduplicationStore)

The dedup store prevents duplicate submissions from reaching the on-chain contract.

**Persistence:** Maintains a persistent record of processed (raffle_contract, request_id) pairs in `./data/dedup.json`.

**Crash Safety Window:**
If the oracle crashes after a successful on-chain submission but before persisting the dedup record, a restart may re-submit the same request. The on-chain contract guards against this by:
1. Emitting a single `provide_randomness` endpoint per request
2. Rejecting duplicate calls with `Error::RandomnessAlreadyProvided`

The dedup store **must be persisted on a durable volume** in production. Without persistent storage, every restart risks re-submitting requests that were already on-chain.

### 6. Checkpoint Store (Ledger Checkpoint)

The checkpoint store records the last successfully processed ledger number.

**Persistence:** Written to `./data/checkpoint.json` after each successful event poll.

**Purpose:** Enables restart recovery by resuming from the last known ledger instead of the genesis block.

**Limitations:** If the last persisted checkpoint is older than the RPC's event retention window (typically 128 ledgers on testnet, more on mainnet), events between the checkpoint and now cannot be recovered and will be silently missed.

## Operational Safety: Data Persistence

The oracle service **requires a persistent volume** for two files:

| File | Purpose | Loss Impact | Mitigation |
|------|---------|-------------|-----------|
| `data/checkpoint.json` | Last processed ledger | Event gaps on restart; missed RandomnessRequested | Resume from network event history if within retention window |
| `data/dedup.json` | Processed (raffle, request_id) pairs | Duplicate submissions re-issued on restart | On-chain dedup guard (`RandomnessAlreadyProvided` error) |

**Production Recommendation:** Mount a persistent volume at `./data` and ensure it survives container/process restarts. Without it, the oracle is stateless and vulnerable to duplicate submissions and missed events.

## Timing and Timeout Interaction

The oracle's timeout behavior interacts with on-chain raffle parameters:

### ORACLE_TIMEOUT_LEDGERS (On-Chain Contract Parameter)

Each raffle specifies a timeout window (in ledgers) during which the oracle must submit a randomness seed. If the timeout expires without submission:
- The raffle transitions to `Drawing` state but cannot finalize
- On the next `finalize_raffle` call, the contract checks if timeout has passed
- If timeout has passed, the contract falls back to internal PRNG seed derivation
- The raffle finalizes with the fallback seed and emits `RandomnessFallback` event

### Oracle Processing Timeline

The oracle polls at an interval (default: 5 seconds, configurable via `POLL_INTERVAL_MS`):

1. **Event observation (polling):** `TimeT` — `RandomnessRequested` is observed
2. **Queue enqueue:** `TimeT+Δ` — Job is added to the in-memory queue
3. **VRF generation:** `TimeT+Δ'` — Seed is computed
4. **RPC submission:** `TimeT+Δ''` — Transaction is sent and confirmed
5. **On-chain effect:** Typically within 1-5 ledgers after transmission

**Relationship to ORACLE_TIMEOUT_LEDGERS:**
- `ORACLE_TIMEOUT_LEDGERS` is measured in ledgers (typically 4-6 seconds per ledger on testnet)
- The oracle's polling interval (5 seconds) should be **much shorter** than the timeout window
- Recommended: `ORACLE_TIMEOUT_LEDGERS >= 30` with a 5-second poll interval gives a 2-minute safety window

## Quorum Mode

In `RandomnessSource::Quorum` mode, multiple oracles participate in a distributed draw:

### Event Flow

1. **Finalization trigger:** The raffle creator calls `finalize_raffle`
2. **Broadcast:** The instance emits a single `RandomnessRequested` event
3. **Fan-out:** The event listener receives the single event and emits per-oracle `RandomnessRequested` events (one per oracle address in the quorum config)
4. **Independent generation:** Each oracle generates its own seed independently
5. **On-chain aggregation:** As each oracle submits `provide_randomness`, its seed is XOR'd into an accumulator
6. **Finalization:** After the quorum threshold is met (typically all oracles, but configurable), the accumulated seed is used for winner selection

### Quorum Submission Tracking

The instance contract maintains a set of oracle addresses that have already submitted:
```
QuorumSubmittedOracles: Vec<Address>
```

Each `provide_randomness` call appends the oracle's address if not already present. Once the quorum size reaches the threshold, the raffle proceeds to winner selection.

### Failure Modes

- **Late/missing oracle:** If an oracle is slower or offline, the raffle waits until the timeout expires, then falls back to internal PRNG
- **Byzantine oracle:** An oracle submitting a deliberately bad seed still contributes to the accumulated entropy (XOR'd); the accumulator remains non-zero regardless
- **Network partition:** If some oracles cannot reach the network but others can, only the reachable oracles contribute

## Alerting and Observability

### Health Endpoint

**`GET /health`** returns `{"status":"ok"}` with HTTP 200 if the service is running. Use this for Kubernetes liveness probes.

### Metrics Endpoint

**`GET /metrics`** returns Prometheus-format metrics. Key metrics:

| Metric | Type | Description |
|--------|------|-------------|
| `oracle_requests_observed_total` | Counter | `RandomnessRequested` events enqueued, labeled by raffle contract |
| `oracle_request_latency_seconds` | Histogram | Wall-clock time from event observation to confirmed on-chain submission |
| `oracle_submissions_total` | Counter | Submission results (`success`, `retry`, `fatal`), labeled by outcome |
| `oracle_queue_depth` | Gauge | Current number of pending randomness jobs |
| `oracle_queue_oldest_age_seconds` | Gauge | Age of the oldest queued job |
| `oracle_dead_letter_total` | Counter | Jobs permanently failed after exhausting retries |
| `oracle_listener_ledger_lag` | Gauge | Ledgers between network tip and last processed checkpoint |
| `oracle_rpc_errors_total` | Counter | RPC errors by phase: `poll`, `simulate`, `send` |
| `oracle_fees_spent_stroops_total` | Counter | Cumulative transaction fees paid for submissions |

### Alert Rules

Suggested Prometheus alert rules are documented in [oracle/RUNBOOK.md](../oracle/RUNBOOK.md#suggested-alert-rules). Common triggers include:

- Queue depth exceeds `ALERT_QUEUE_DEPTH_LIMIT`
- Oldest queued job exceeds `ALERT_QUEUE_AGE_LIMIT_MS`
- RPC polling failures reach `ALERT_RPC_UNREACHABLE_THRESHOLD`
- Submission failures reach `ALERT_FAILURE_THRESHOLD`

### Webhook Alerting

If `ALERT_WEBHOOK_URL` is set (e.g., Slack, Discord, PagerDuty webhook), the oracle emits a generic JSON POST body:

```json
{
  "type": "submission_failure",
  "severity": "critical",
  "message": "3 consecutive provide_randomness submissions failed",
  "timestamp": 1700000000000,
  "details": { "consecutiveFailures": 3, "threshold": 3 }
}
```

Alerts are rate-limited per type to prevent webhook storms.

## Configuration

For a complete list of environment variables, see [oracle/README.md](../oracle/README.md#required-environment-variables).

### Critical Production Variables

| Variable | Default | Purpose |
|----------|---------|---------|
| `ORACLE_SECRET_KEY` | — | Ed25519 secret key for transaction signing |
| `STELLAR_RPC_URL` | — | Soroban RPC endpoint (e.g., `https://soroban-testnet.stellar.org`) |
| `FACTORY_CONTRACT_ID` | — | Raffle factory contract address; events are polled from this factory |
| `ORACLE_RETRY_BASE_MS` | 500 | Retry backoff base in milliseconds |
| `ORACLE_RETRY_MAX_MS` | 30000 | Maximum retry backoff in milliseconds |
| `ORACLE_RETRY_MAX_ATTEMPTS` | 5 | Maximum number of submission retry attempts |
| `POLL_INTERVAL_MS` | 5000 | Event polling interval in milliseconds |
| `LOG_LEVEL` | `info` | Log verbosity (`debug`, `info`, `warn`, `error`) |

See [oracle/README.md](../oracle/README.md) for the full configuration reference and production key management.

## Integration with On-Chain Contracts

### RandomnessRequested Event

Emitted by the raffle instance when `finalize_raffle` is called:

```rust
pub struct RandomnessRequested {
    pub oracle: Address,          // Single oracle address (External mode)
    pub request_id: u64,          // Unique request ID for this raffle
    pub timestamp: u64,           // Ledger timestamp of emission
}
```

In Quorum mode, the listener receives one event and fans it out to each oracle in the quorum.

### provide_randomness Entrypoint

Called by the oracle to submit the computed seed:

```rust
pub fn provide_randomness(
    env: Env,
    request_id: u64,
    seed: u64,
    proof: Option<Bytes>,
) -> Result<(), Error>
```

For External mode, the oracle provides a signature proof. For Quorum mode, the proof is optional (may be `None`).

### Fallback Behavior

If no seed is received within `ORACLE_TIMEOUT_LEDGERS`, the raffle automatically finalizes using an internal PRNG seed and emits `RandomnessFallback` event. This ensures raffles never stall indefinitely due to oracle unavailability.

## Deployment Patterns

### Single Oracle (External Mode)

1. Deploy one oracle instance with the raffle factory contract address
2. Register the oracle's public key on-chain via factory admin config
3. Configure `ORACLE_SECRET_KEY` securely (environment variable or secrets manager)
4. Start the service and monitor `/metrics` and `/health`

### Quorum Setup (Multiple Oracles)

1. Deploy N oracle instances, each with the same factory contract address
2. Register all N oracle public keys in the raffle's quorum config on-chain
3. Each oracle independently polls and generates seeds
4. The contract XORs seeds as they arrive
5. Once quorum threshold is reached, finalization proceeds

## Monitoring Checklist

Use this checklist when deploying or troubleshooting an oracle:

- [ ] `/health` returns `{"status":"ok"}` and HTTP 200
- [ ] `/metrics` is accessible and contains `oracle_*` metrics
- [ ] `oracle_requests_observed_total` counter increments when a raffle is finalized
- [ ] `oracle_queue_depth` is 0 (or small) during normal operation
- [ ] `oracle_listener_ledger_lag` is < 5 (oracle is close to network tip)
- [ ] `oracle_rpc_errors_total` is not increasing rapidly
- [ ] `oracle_submissions_total{outcome="success"}` counter increments for successful submissions
- [ ] `/data/checkpoint.json` and `/data/dedup.json` are persisted on a durable volume
- [ ] `ORACLE_SECRET_KEY` is configured securely (not in logs)
- [ ] Log level is appropriate for your environment (default: `info`)
- [ ] Alerts are wired to your monitoring system (webhook URL set)

## See Also

- [oracle/README.md](../oracle/README.md) — Configuration, key management, dependency policy
- [oracle/RUNBOOK.md](../oracle/RUNBOOK.md) — Operational procedures, alerting, crash-safety details
- [docs/RANDOMNESS.md](RANDOMNESS.md) — Randomness modes and security properties
- [docs/ARCHITECTURE.md](ARCHITECTURE.md) — System overview and raffle state machine
