# Tikka Protocol Fee Model

Protocol fees are charged at ticket purchase and prize claim. Basis-point
amounts use floor division consistently at both sites.

## Rounding Rules

| Site | Formula | Rounding |
|------|---------|----------|
| Ticket purchase | `total_price × protocol_fee_bp / 10_000` | Floor |
| Prize claim | `tier_amount × protocol_fee_bp / 10_000` | Floor |

Ticket-purchase fees are held in the contract and recorded in `AccumulatedFees`
until an admin withdraws them after finalization. Prize-claim fees are
transferred directly to the treasury and are not added to `AccumulatedFees`.
Each fee is therefore paid out exactly once.

## Tier Prize Allocation

- **Formula:** `prize_amount × tier_basis_points / 10_000` for every tier except the final tier.
- **Final tier:** Receives `prize_amount` minus the amounts allocated to all earlier tiers.
- **Rounding rule:** Integer-division dust is assigned to the final tier, so all tier prizes sum exactly to `prize_amount` and no prize funds remain undistributed.

## Worked Example (mirrored by `fee_model_worked_example_matches_lifecycle`)

Parameters:

- `protocol_fee_bp = 250` (2.5 %)
- `ticket_price = 100_000_000` stroops (100 XLM)
- `max_tickets = 10` (all sold)
- `prize_amount = 800_000_000` stroops (800 XLM, single tier)

### Ticket purchase fees (floor)

Per ticket: `100_000_000 × 250 / 10_000 = 2_500_000` stroops (2.5 XLM)

Total ticket fees: `10 × 2_500_000 = 25_000_000` stroops (25 XLM)

### Prize claim fee (floor)

Single tier gross: `800_000_000` stroops

Claim fee: `800_000_000 × 250 / 10_000 = 20_000_000` stroops (20 XLM)

Winner receives: `800_000_000 − 20_000_000 = 780_000_000` stroops (780 XLM)

### Total protocol revenue

| Source | Amount (stroops) | Amount (XLM) |
|--------|------------------|--------------|
| Ticket fees | 25_000_000 | 25 |
| Claim fee | 20_000_000 | 20 |
| **Total** | **45_000_000** | **45** |

The end-to-end invariant test in
`contracts/raffle-instance/src/tests/invariants.rs` walks this lifecycle and
asserts the treasury balance increases by exactly 45 XLM — no double-count via
`withdraw_fees`, and no rounding drift.

## Fee Collection Points

### 1. At Ticket Purchase

- **Formula:** `total_price × protocol_fee_bp / 10_000` (floor division)
- **Recipient:** Accrued in the contract; later paid to the recipient selected by the admin
- **Payer:** Ticket buyer

### 2. At Prize Claim

- **Formula:** `prize_tier_amount × protocol_fee_bp / 10_000` (floor division)
- **Recipient:** Treasury address
- **Payer:** Prize winner (deducted from payout)
- **Accounting:** Transferred immediately; not included in `AccumulatedFees`

## Zero Fee Rate

When `protocol_fee_bp = 0`, no fees are collected at either site and the
treasury balance must not change across the full lifecycle.

## Maximum Fee Rate

`MAX_PROTOCOL_FEE_BP = 2_000` (20 %). The invariant test repeats the full
lifecycle at `protocol_fee_bp ∈ {0, 1, 100, 2_000}`.
