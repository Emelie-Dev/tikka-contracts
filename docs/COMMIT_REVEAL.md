# Commit-Reveal Randomness Protocol

> **⚠️ Status: Partially Implemented**
>
> The following features are **currently implemented**: Commit phase and Draw phase.
>
> The following features are **planned and not yet implemented**:
> - **Reveal Phase**: Entrypoint for winners to reveal their original secrets for cryptographic verification
> - **Ticket Transfer**: Transfer tickets between holders while preserving commit entropy
>
> See [#1083](https://github.com/crackedstudio/tikka-contracts/issues/1083) for tracking.

This document explains the `RandomnessSource::CommitReveal` protocol, a multi-phase approach to generating fair, verifiable randomness for raffles. The commit and draw phases are production-ready; the reveal and transfer phases are planned.

## 1. Protocol Steps

The Commit-Reveal raffle lifecycle consists of four distinct phases:

### 1.1 Raffle Creation

The raffle creator initializes the raffle with `randomness_source = CommitReveal`.

### 1.2 Commit Phase (Active State)

During the raffle's Active state:

- Each ticket buyer generates a local secret: `secret = random_bytes(32)`
- The buyer computes a cryptographic hash: `hash = sha256(secret)`
- The buyer calls `submit_commit(ticket_id, hash)` to commit their hash on-chain

### 1.3 Draw Phase (finalize_raffle) ✓ Implemented

When `finalize_raffle` is called:

- The contract queries all existing `CommitEntry(ticket_id)` records
- All collected hashes are concatenated and hashed sequentially: `combined = sha256(hash_1 || hash_2 || ... || hash_n)`
- The first 8 bytes of the `combined` hash are extracted and used as the final draw seed

### 1.4 Reveal Phase ⏳ Planned (Not Yet Implemented)

After the raffle finalizes, winners will be able to call `reveal_commit(ticket_id, secret)` on-chain to mathematically prove the entropy generation was honest and unmanipulated. Currently, reveal validation can only be performed off-chain.

## 2. Ticket Transfer Invariant ⏳ Planned (Not Yet Implemented)

The ticket transfer mechanism is planned but not yet implemented. When available, it will preserve entropy through the following invariant:

Commit entries are **structurally keyed by ticket ID**, not by the owner's public address. This means:

- A commit submitted by the original buyer will remain entirely intact and preserved even if the associated ticket is transferred or traded before finalization occurs
- This prevents the silent loss of entropy when tickets change hands

### 2.1 One commit per ticket ✓ Implemented

`submit_commit` stores at most one `CommitEntry` per `ticket_id`.

- A second submission for the same ticket returns `Error::CommitAlreadySubmitted`.
- This blocks last-look bias: a participant cannot overwrite their commitment after
  seeing other on-chain commits and recomputing the prospective seed.

### 2.2 Commit window ✓ Implemented

Commits are accepted only while the raffle status is **`Active`**.

Once the raffle enters **`Drawing`**, the commit window is closed. Accepting new
entropy after the draw is triggered would give the last committer more information
than earlier participants.

## 3. Fallback Behavior

If zero commits are submitted by the time finalization is triggered, the contract automatically falls back to using an internal PRNG fallback mechanism so the raffle can still be finalized.

## 4. Currently Available Features

### 4.1 Commit Hash Generation

Ticket holders can generate a commit hash locally using standard cryptographic libraries:

#### TypeScript Example

```typescript
import crypto from 'crypto';

function generateCommitHash(): { secret: Buffer; hash: Buffer } {
  const secret = crypto.randomBytes(32);
  const hash = crypto.createHash('sha256').update(secret).digest();
  return { secret, hash };
}

// Usage
const { secret, hash } = generateCommitHash();
// Submit `hash` on-chain via submit_commit(ticket_id, hash)
// Store `secret` securely for future use
```

#### Rust Example

```rust
use sha2::{Sha256, Digest};
use rand::RngCore;

fn generate_commit_hash() -> (Vec<u8>, Vec<u8>) {
    let mut secret = vec![0u8; 32];
    rand::thread_rng().fill_bytes(&mut secret);
    
    let mut hasher = Sha256::new();
    hasher.update(&secret);
    let hash = hasher.finalize().to_vec();
    
    (secret, hash)
}

// Usage
let (secret, hash) = generate_commit_hash();
// Submit `hash` on-chain via submit_commit(ticket_id, hash)
// Store `secret` securely for future use
```

## 5. Planned Features

The following sections describe features that are planned but not yet implemented. Do not write client code that depends on these endpoints.

### 5.1 Reveal Phase

After raffle finalization, winners will be able to call `reveal_commit(ticket_id, secret)` to prove their entropy contribution was honest. This entrypoint does not yet exist.

### 5.2 Ticket Transfer

Transferring tickets to other holders while preserving their commit entropy is planned but not yet implemented. There is currently no transfer entrypoint.
