# Fee Model

## Rounding direction

**All basis-point calculations in this protocol truncate (floor division).**

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

## Property guarantee

For all `amount: i128` and all `bp` in `0..=10_000`:

```
let (fee, remainder) = split_bp(amount, bp).unwrap();
assert_eq!(fee + remainder, amount);
```

This is verified by the exhaustive test
`split_bp_sum_property_all_bp_values` in `contracts/raffle-shared/src/math.rs`.
