# Fee Model

Protocol fees are charged at ticket purchase and prize claim. Basis-point
amounts use floor division consistently at both sites.
## Rounding direction

**All basis-point calculations in this protocol truncate (floor division).**

| Site | Formula | Rounding |
|------|---------|----------|
| Ticket purchase | `total_price × protocol_fee_bp / 10_000` | Floor |
| Prize claim | `tier_amount × protocol_fee_bp / 10_000` | Floor |

Ticket-purchase fees are held in the contract and recorded in `AccumulatedFees`
until an admin withdraws them after finalization. Prize-claim fees are
transferred directly to the treasury and are not added to `AccumulatedFees`.
Each fee is therefore paid out exactly once.
This means the protocol collects *at most* the stated percentage — never more.
Truncation always favours the payer (ticket buyer or prize winner), which is
the correct default for a fair raffle protocol.

## Implementation

Every fee and prize calculation goes through one of two functions in
`contracts/raffle-shared/src/math.rs`:

| Function | Formula | Returns |
|---|---|---|
| `apply_bp(amount, bp)` | `floor(amount × bp / 10_000)` | the fee/share |
| `split_bp(amount, bp)` | `(floor(amount × bp / 10_000), amount − fee)` | `(fee, remainder)` |

Use `split_bp` whenever you need both sides of a split (e.g. fee and net
payout) so that `fee + remainder == amount` exactly — no rounding gap.

The denominator `10_000` is the constant `BP_DENOMINATOR` exported from the
same module.

## Where fees are charged

| Event | Formula | Rounding |
|---|---|---|
| Ticket purchase | `floor(net_price × protocol_fee_bp / 10_000)` | truncate |
| Early-bird discount | `floor(gross × early_bird_discount_bp / 10_000)` | truncate |
| Prize claim | `floor(prize_amount × protocol_fee_bp / 10_000)` | truncate |
| Prize tier split | `floor(prize_amount × tier_bp / 10_000)` | truncate |

## Numeric safety

- All intermediate products use `checked_mul` to catch `i128` overflow before
  it occurs.
- `MAX_PRIZE_AMOUNT` (1e21) and `MAX_PROTOCOL_FEE_BP` (2 000) are chosen so
  that `MAX_PRIZE_AMOUNT × MAX_PROTOCOL_FEE_BP` fits in `i128`
  (`2 × 10^24 < 1.7 × 10^38`).

### Prize claim fee (floor)
## Property guarantee

For all `amount: i128` and all `bp` in `0..=10_000`:

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
```
let (fee, remainder) = split_bp(amount, bp).unwrap();
assert_eq!(fee + remainder, amount);
```

This is verified by the exhaustive test
`split_bp_sum_property_all_bp_values` in `contracts/raffle-shared/src/math.rs`.
