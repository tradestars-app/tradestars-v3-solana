# TradeStars Arena

TradeStars mirrors Base deposits into Solana `tUSDC`, keeps `tUSDC` soulbound with Token-2022 `NonTransferable`, and enforces arena participation with burn-on-commit accounting.

## Core model

- `deposit_collateral`
  Only the configured `minting_authority` may mint mirrored deposits. The instruction creates a replay marker for `(base_tx_hash, log_index)`, mints `tUSDC`, and increases `UserAccount.total_balance`.
- `create_arena`
  The arena operator and creator co-sign. If the arena has a guaranteed prize, the creator burns that amount immediately and the program locks the same amount in the creator's `in_play_debt`.
- `join_arena`
  The user burns the full `entry_fee`, the program increases `in_play_debt`, records `fee_accrued`, and adds only the net entry amount to `total_pool`.
- `post_settlement_root`
  The operator posts one immutable Merkle root after `end_time`. That starts the dispute window.
- `submit_dispute`
  Each participating user may dispute once per settlement version. If disputes reach `>= 5%` of unique participants, the arena moves to `Disputed`.
- `cancel_disputed_arena`
  Anyone may cancel a disputed arena. Disputed arenas refund; they do not accept corrected roots.
- `cancel_stale_arena`
  If the operator never posts a root by `end_time + settlement_grace_period_seconds`, anyone may cancel and route the arena into refunds.
- `settle_arena_batch` / `claim_winnings`
  After `claimable_at`, the operator settles users in batches by default. Missed users can still claim directly.
- `refund_arena_batch` / `claim_refund`
  Cancelled arenas are refunded in operator batches by default, with a user fallback path.
- `finalize_arena`
  Anyone may finalize once all positions are resolved. Settled finalization mints fees to treasury and returns any unused creator guarantee. Cancelled finalization returns the full creator guarantee and mints no fees.
- `withdraw_request`
  Burns `tUSDC`, decreases `total_balance`, and emits the replay-safe withdrawal event for the Base release flow.

## Fee lifecycle

Entry fees are collected at `join_arena`.

- The full `entry_fee` is burned when the user joins.
- `fee_amount = floor(entry_fee * fee_bps / 10_000)`.
- `fee_accrued += fee_amount` immediately.
- Only `entry_fee - fee_amount` is added to `total_pool`.
- If the arena is cancelled, users are refunded their full committed amount and no fee is minted.
- If the arena settles, `fee_accrued` is minted to the treasury only on `finalize_arena`.

## Main invariants

- `available_to_withdraw = total_balance - in_play_debt`
- `tUSDC` cannot be peer-transferred
- each `(base_tx_hash, log_index)` deposit can be processed once
- each user can enter the same arena up to `10` times
- guaranteed prizes are locked at arena creation, not funded later
- joins and operator cancellation are only allowed before `start_time`
- a settlement root can be posted only once, only after `end_time`
- `total_pool = guaranteed_prize_reserved + (total_entry_fees_locked - fee_accrued)`
- `total_claimed_payout <= total_pool`
- disputed arenas can only cancel and refund
- cancelled arenas refund the full committed amount
- fees are minted to treasury only when a settled arena is finalized

## Trust assumptions

- Base deposit mirroring is centralized in this version.
- The configured `minting_authority` is trusted to call `deposit_collateral` only for real Base deposits.
- The replay marker prevents duplicate processing of the same `(base_tx_hash, log_index)`, but the program does not verify a Base event proof on-chain.
- The configured `arena_operator` is trusted to create arenas and publish the settlement root.
- Disputes have one on-chain effect in this version: if they reach `>= 5%`, the arena becomes `Disputed` and must refund.

## Local commands

```bash
cargo build
anchor build
anchor test
```

Detailed architecture and Mermaid diagrams are in [docs/vault-architecture.md](docs/vault-architecture.md).
