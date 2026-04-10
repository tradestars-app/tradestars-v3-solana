# TradeStars Arena (Solana)

This repo contains the Solana program for TradeStars arena custody and settlement.

Current design goals:
- no legacy/backward-compat layer
- guaranteed prize support
- claimless UX for winners and refunds
- minimal entrypoint surface

All AMM pricing, scoring, and leaderboard computation remain off-chain.

## Scope

On-chain responsibilities:
- collect entry fees in per-arena vaults
- custody funds and collect platform fees
- auto-fund overlay when guaranteed prize exceeds collected net
- auto-pay winners in settlement batches
- auto-refund users in cancellation batches
Off-chain responsibilities:
- trading engine / AMM
- scoring and final ranking
- payout plan generation

## Accounts

- `PlatformConfig`: authority, fee recipient, fee bps, pause flag
- `Arena`: pool accounting, settlement status, authority reference
- `ArenaEntry`: participation proof + payout/refund settlement flags

### PDA Seeds

| Account | Seeds |
|---------|-------|
| `PlatformConfig` | `["config"]` |
| `platform_vault` | `["platform_vault"]` |
| `Arena` | `["arena", arena_id]` |
| `arena_vault` | `["vault", arena_pubkey]` |
| `ArenaEntry` | `["entry", arena_pubkey, user_pubkey, entry_number]` |

## Entrypoints

| Instruction | Signer | Description |
|-------------|--------|-------------|
| `initialize_platform` | authority | One-time setup: config PDA, platform vault |
| `create_arena` | authority | Create arena + per-arena vault with validated params |
| `enter_arena` | user | Pay entry fee, create entry PDA |
| `settle_and_pay_batch` | authority | Finalize, fund overlay, pay winners in batch |
| `cancel_arena` | authority | Transition arena from Open to Cancelled |
| `refund_batch` | authority | Batch refund entries for cancelled arenas |
| `withdraw_fees` | authority | Withdraw collected fees from platform vault |
| `toggle_pause` | authority | Toggle platform pause flag |

## State Machine

```
Open ──(end_time reached + settle_and_pay_batch)──> Finalized ──(all payouts done)──> Settled
Open ──(cancel_arena)──> Cancelled
```

- `Open`: accepting entries (before `end_time`)
- `Finalized`: fee collected, overlay funded, payouts in progress
- `Settled`: terminal state, all payouts complete
- `Cancelled`: terminal state, refunds in progress

## Settlement Model

`settle_and_pay_batch` does all settlement-critical actions:
1. Collect platform fee (first call only, transitions Open to Finalized)
2. Fund missing overlay from authority's token account (if guaranteed prize > collected net)
3. Pay winners from arena vault via `remaining_accounts` pairs (entry + user token account)
4. Persist `payout_amount` + `settled` on each paid entry
5. Emit `WinnerPaidEvent` with rank for audit transparency
6. Auto-transition to `Settled` when `total_payouts_set >= max_distributable`

Ranks are not stored in account state; they are emitted in `WinnerPaidEvent` and remain off-chain source-of-truth data.

Idempotency: re-submitting an already-settled entry in a batch is a no-op (skipped with `continue`) as long as the payout amount matches.

## Cancellation Model

`refund_batch` is authority-driven and claimless:
- Admin submits entry/user-token pairs via `remaining_accounts`
- Contract transfers full `amount_paid` refund to each user
- Marks each refunded entry as `settled`
- Already-settled entries are skipped (idempotent)
- Total refunds bounded by `total_pool`

## Invariants

- `total_payouts_set <= max_distributable`
- `max_distributable = (total_pool - fee_amount) + overlay_funded`
- `total_refunds_paid <= total_pool` (cancellation path)
- Arena moves to `Settled` when `total_payouts_set >= max_distributable`
- Off-chain payout plan must always sum to exactly `max_distributable`
- Fee collection happens exactly once per arena (`fee_collected` flag)
- Each entry is settled at most once (`settled` flag as replay protection)
- All `remaining_accounts` entry accounts must be writable (enforced on-chain)
- Arena authority must match platform authority for cancel/settle/refund operations
- `arena_vault` token account owner is always the arena PDA

## Input Validation

- `arena_id`: 1-64 bytes
- `entry_fee`: > 0
- `guaranteed_prize_pool`: > 0
- `max_entries_per_user`: > 0
- `end_time`: must be in the future and > `start_time`
- `platform_fee_bps`: <= 10,000

## Quick Start

```bash
pnpm install
anchor build
anchor test
```

## Deployment

### Fresh deploy (new cluster or first time)

```bash
# 1. Build program + IDL
anchor build

# 2. Deploy to target cluster
anchor deploy --provider.cluster devnet

# 3. Initialize platform (creates PlatformConfig + platform vault)
cd ../tradestars-v3
npx tsx scripts/initialize-platform.ts

# 4. Copy updated IDL types to the web app
cp ../tradestars-v3-solana/target/types/tradestars_arena.ts src/lib/solana/types/tradestars_arena.ts
```

### Upgrade deploy (PlatformConfig layout changed)

When `PlatformConfig` fields change (e.g. adding `usdc_mint`), the old account
can't be deserialized by the new program. You must close and re-initialize:

```bash
# 1. Build and deploy the new program
anchor build
anchor deploy --provider.cluster devnet

# 2. Derive PDA addresses
#    PlatformConfig: seeds = ["config"]
#    PlatformVault:  seeds = ["platform_vault"]

# 3. Close the old platform vault (SPL token account)
spl-token close <PLATFORM_VAULT_PDA> --url devnet

# 4. Close the old PlatformConfig account (reclaim rent to authority)
solana close <PLATFORM_CONFIG_PDA> --url devnet

# 5. Re-initialize the platform
cd ../tradestars-v3
npx tsx scripts/initialize-platform.ts

# 6. Copy updated IDL types to the web app
cp ../tradestars-v3-solana/target/types/tradestars_arena.ts src/lib/solana/types/tradestars_arena.ts
```

**Important:** Any existing arenas created under the old program will still
reference the old `PlatformConfig` layout. If the platform is not yet live,
this is fine — old arenas can be abandoned. If live, you need a migration
strategy before upgrading.
