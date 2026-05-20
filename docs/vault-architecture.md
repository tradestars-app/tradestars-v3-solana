# Vault Architecture

## Overview

TradeStars uses Solana as the enforcement layer for mirrored balances, arena commitments, disputes, settlements, refunds, and withdrawal replay protection.

This version is intentionally simple:

- Base custody stays on Base.
- Solana mints `tUSDC` 1:1 through a dedicated Solana `minting_authority`.
- `tUSDC` is Token-2022 `NonTransferable`.
- Arena commitments are represented by immediate burns plus on-chain debt accounting.
- Guaranteed prizes are locked by the arena creator at creation time.
- The operator posts one immutable settlement root per arena.
- If disputes reach `>= 5%`, the arena becomes `Disputed` and can only refund.
- If the operator never settles, anyone can stale-cancel after the grace period.
- Once all positions are resolved, anyone can finalize.

There is no per-arena token vault.

## Base deposit flow

The Base contract and backend are still the source of truth for deposits, but Solana does not verify Base proofs on-chain in this version.

The flow is:

1. A user deposits USDC into the Base deposit contract.
2. The backend watches Base and decides the deposit is valid.
3. The backend calls `deposit_collateral` using the configured Solana `minting_authority`.
4. The Solana program checks:
   - caller is `minting_authority`
   - `deposits_paused == false`
   - replay marker for `(base_tx_hash, log_index)` does not already exist
5. The program mints `tUSDC` and increases `UserAccount.total_balance`.

This is a centralized minting model.

## Sequence diagrams

### Deposit

```mermaid
sequenceDiagram
    participant U as User
    participant B as Base Deposit Contract
    participant S as Backend
    participant M as Minting Authority
    participant P as Solana Program
    participant T as User tUSDC ATA

    U->>B: Deposit USDC on Base
    B-->>S: Emit deposit event
    S->>M: Request mint on Solana
    M->>P: deposit_collateral(user, amount, base_tx_hash, log_index)
    P->>P: Require signer == minting_authority
    P->>P: Require deposits_paused == false
    P->>P: Create replay marker for (base_tx_hash, log_index)
    P->>P: Create or load UserAccount
    P->>T: Mint tUSDC
    P->>P: total_balance += amount
```

### Withdraw

```mermaid
sequenceDiagram
    participant U as User
    participant P as Solana Program
    participant T as User tUSDC ATA
    participant L as Off-chain Listener
    participant B as Base Release Flow

    U->>P: withdraw_request(amount, nonce)
    P->>P: Check nonce
    P->>P: Check amount <= total_balance - in_play_debt
    P->>T: Burn tUSDC
    P->>P: total_balance -= amount
    P-->>L: Emit WithdrawRequested
    L->>B: Release Base USDC
    B-->>U: User receives USDC on Base
```

### Create arena and lock guarantee

```mermaid
sequenceDiagram
    participant O as Arena Operator
    participant C as Arena Creator
    participant P as Solana Program
    participant T as Creator tUSDC ATA

    O->>P: create_arena(arena_id, params)
    C->>P: Co-sign arena creation
    P->>P: Validate start_time and end_time
    alt guaranteed_prize_target > 0
        P->>P: Check creator available balance
        P->>T: Burn guaranteed_prize_target
        P->>P: creator.in_play_debt += guaranteed_prize_target
        P->>P: arena.guaranteed_prize_reserved = guaranteed_prize_target
        P->>P: arena.total_pool += guaranteed_prize_target
    end
    P->>P: Arena status = Created
```

### Join arena and accrue fees

```mermaid
sequenceDiagram
    participant U as User
    participant P as Solana Program
    participant T as User tUSDC ATA

    U->>P: join_arena(arena_id)
    P->>P: Check arena is Created
    P->>P: Check now < start_time
    P->>P: Check available balance
    P->>T: Burn entry_fee
    P->>P: user.in_play_debt += entry_fee
    P->>P: position.entry_count += 1
    P->>P: position.locked_amount += entry_fee
    P->>P: fee_accrued += fee_amount
    P->>P: total_pool += entry_fee - fee_amount
    Note over P: Fees are accounted on join but minted to treasury only on settled finalize
```

### One immutable settlement root

```mermaid
sequenceDiagram
    participant O as Arena Operator
    participant U as User
    participant P as Solana Program

    O->>P: post_settlement_root(arena_id, merkle_root)
    P->>P: Require now >= end_time
    P->>P: settlement_version += 1
    P->>P: claimable_at = now + dispute_window_seconds
    U->>P: submit_dispute()
    P->>P: One dispute per participant per version
    alt disputes >= 5% of participants
        P->>P: status = Disputed
    else dispute threshold not reached
        Note over P: Arena remains SettledPendingClaim
    end
```

### Settle or fallback claim

```mermaid
sequenceDiagram
    participant O as Arena Operator
    participant U as User
    participant P as Solana Program
    participant T as User tUSDC ATA

    Note over P: claimable_at has elapsed and arena is still SettledPendingClaim
    O->>P: settle_arena_batch(entries...)
    P->>P: Verify proof and pool bounds
    P->>P: total_balance = total_balance - locked + payout
    P->>P: in_play_debt -= locked
    P->>T: Mint payout
    Note over U,P: Any unresolved user can still call claim_winnings(...)
```

### Disputed or stale cancellation

```mermaid
sequenceDiagram
    participant X as Any Caller
    participant P as Solana Program

    alt arena is Disputed
        X->>P: cancel_disputed_arena(arena_id)
        P->>P: status = Cancelled
    else no root was posted by grace deadline
        X->>P: cancel_stale_arena(arena_id)
        P->>P: Require now >= end_time + settlement_grace_period_seconds
        P->>P: status = Cancelled
    end
```

### Refund and finalize

```mermaid
sequenceDiagram
    participant O as Arena Operator
    participant U as User
    participant X as Any Caller
    participant P as Solana Program
    participant T as User tUSDC ATA
    participant C as Creator tUSDC ATA
    participant F as Treasury tUSDC ATA

    O->>P: refund_arena_batch(users...)
    P->>P: in_play_debt -= locked_amount
    P->>T: Mint locked_amount back
    Note over U,P: Any missed user can still call claim_refund(...)
    X->>P: finalize_arena(arena_id)
    alt arena settled
        P->>C: Mint unused guarantee
        P->>F: Mint fee_accrued
    else arena cancelled
        P->>C: Mint full creator guarantee back
    end
    P->>P: status = Finalized
```

## Authority model

- `authority`
  Initializes the platform, updates config, and can pause or resume deposits.
- `minting_authority`
  The only signer allowed to call `deposit_collateral`.
- `arena_operator`
  Creates arenas, posts settlement roots, settles batches, cancels pre-start arenas, and refunds batches.
- `treasury_wallet`
  Receives fees minted on settled finalization.

There is no multisig, attester signature verification, or on-chain root rewrite path in this version.

## Accounts

### `PlatformConfig`

Seeds:

```text
["config"]
```

Fields:

- `authority`
- `arena_operator`
- `treasury_wallet`
- `minting_authority`
- `dispute_window_seconds`
- `settlement_grace_period_seconds`
- `deposits_paused`
- `bump`

### `UserAccount`

Seeds:

```text
["user", user_pubkey]
```

Fields:

- `owner`
- `total_balance`
- `in_play_debt`
- `last_nonce`
- `bump`

Rule:

```text
available_to_withdraw = total_balance - in_play_debt
```

### `ArenaAccount`

Seeds:

```text
["arena", arena_id]
```

Fields:

- `arena_id`
- `creator`
- `status`
- `entry_fee`
- `fee_bps`
- `guaranteed_prize_target`
- `guaranteed_prize_reserved`
- `total_entry_fees_locked`
- `fee_accrued`
- `total_pool`
- `total_claimed_payout`
- `start_time`
- `end_time`
- `merkle_root`
- `settlement_timestamp`
- `claimable_at`
- `participant_count`
- `resolved_count`
- `dispute_count`
- `settlement_version`
- `metadata_hash`
- `bump`

Derived values:

```text
net_entry_pool = total_entry_fees_locked - fee_accrued
settlement_payout_cap = max(net_entry_pool, guaranteed_prize_reserved)
guaranteed_prize_used = max(total_claimed_payout - net_entry_pool, 0)
unused_guarantee = guaranteed_prize_reserved - guaranteed_prize_used
```

### `ArenaPosition`

Seeds:

```text
["position", arena_pubkey, user_pubkey]
```

Fields:

- `user`
- `arena`
- `entry_count`
- `locked_amount`
- `resolved`
- `last_disputed_settlement_version`
- `bump`

This is the canonical record that a specific user joined a specific arena.

## Arena lifecycle

```mermaid
stateDiagram-v2
    [*] --> Created
    Created --> Cancelled: operator cancel before start_time
    Created --> SettledPendingClaim: post_settlement_root after end_time
    Created --> Cancelled: cancel_stale_arena after grace period
    SettledPendingClaim --> Disputed: disputes reach >= 5%
    SettledPendingClaim --> Finalized: all positions resolved, anyone finalizes
    Disputed --> Cancelled: cancel_disputed_arena
    Cancelled --> Finalized: all positions resolved, anyone finalizes
```

## Instruction summary

### `deposit_collateral`

- minting-authority only
- rejected while `deposits_paused == true`
- replay-safe on `(base_tx_hash, log_index)`

### `create_arena`

- operator plus creator co-sign
- guaranteed prize is burned and locked immediately at creation

### `join_arena`

- user-signed
- allowed only while `status == Created` and `now < start_time`
- burns the full `entry_fee`

### `post_settlement_root`

- operator-only
- allowed only once, only after `end_time`
- starts the dispute window

### `submit_dispute`

- one dispute per participant per settlement version
- allowed only while `now < claimable_at`
- if disputes reach `>= 5%`, status becomes `Disputed`

### `cancel_stale_arena`

- permissionless
- allowed only from `Created`
- allowed only after `end_time + settlement_grace_period_seconds`

### `cancel_disputed_arena`

- permissionless
- allowed only from `Disputed`

### `settle_arena_batch`

- operator-only default payout path
- requires `status == SettledPendingClaim` and `now >= claimable_at`
- canonical PDA and canonical ATA checks are enforced in batch inputs

### `claim_winnings`

- user fallback payout path
- same settlement checks as the batch path

### `refund_arena_batch`

- operator default refund path for cancelled arenas

### `claim_refund`

- user fallback refund path for cancelled arenas

### `finalize_arena`

- permissionless once all positions are resolved
- settled finalize mints fees to treasury and unused guarantee back to creator
- cancelled finalize returns the full creator guarantee and mints no fees

## Security and trust notes

- Deposits are centralized. The configured `minting_authority` is trusted to mirror only real Base deposits.
- The replay marker prevents duplicate minting for the same `(base_tx_hash, log_index)`.
- The arena operator is trusted to publish the settlement root.
- The contract enforces payout bounds, replay protection, canonical PDAs, canonical ATAs in batch paths, and settlement/refund one-time resolution.
- Disputes do not produce corrected roots in this version. A disputed arena refunds instead.
