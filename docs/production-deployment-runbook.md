# Production Deployment Runbook

This runbook is the checklist for deploying and initializing the TradeStars
Solana program in production. Follow it exactly; most production mistakes come
from using the wrong program id, initializing with the wrong role keys, or
deploying code that the application/relayer is not configured to use.

## Security model

Production has four separate operational roles:

- `upgrade_authority`: controls program upgrades. This should be a cold or
  hardware-backed key. It should not be used by the relayer or web app.
- `authority`: controls platform config updates and deposit pause/resume. This
  should be an admin operations key with tightly controlled access.
- `arena_operator`: creates arenas, posts settlement roots, runs batch
  settlement, cancels pre-start arenas, and runs batch refunds.
- `minting_authority`: relayer key allowed to call `deposit_collateral` after a
  finalized Base deposit has been verified.
- `treasury_wallet`: receives protocol fees on settled finalization.

For the MVP, these can technically be the same wallet, but production should not
do that. At minimum, keep `minting_authority` separate from `authority` and
`upgrade_authority`.

## Program id rules

`Anchor.toml` and `programs/tradestars-arena/src/lib.rs` must agree before
building:

- `Anchor.toml` is the source of truth for `[programs.mainnet]`.
- `declare_id!(...)` must match the production program id.
- Do not rely on `target/deploy/tradestars_arena-keypair.json` as the source of
  truth. It is a local build artifact and can be stale.
- Do not use `anchor deploy` for production in this repo.
- Deploy or upgrade explicitly with `solana program deploy --program-id`.

Before production build:

```bash
rg -n "tradestars_arena =|declare_id" Anchor.toml programs/tradestars-arena/src/lib.rs
```

Both values must match the intended production program id.

## Pre-deploy checklist

1. Confirm the exact git commit to deploy.
2. Confirm `Anchor.toml` production program id.
3. Confirm `declare_id!` matches the production program id.
4. Run local tests.
5. Build the program from a clean checkout.
6. Record the SHA256 hash of the built `.so`.
7. Confirm the deploy wallet is the current upgrade authority if this is an
   upgrade.
8. Confirm all production role public keys are available.
9. Confirm the web app and relayer production env values are ready but not yet
   switched until post-deploy verification passes.

Commands:

```bash
pnpm build
pnpm test
shasum -a 256 target/deploy/tradestars_arena.so
solana config get
```

Use an explicit RPC URL in every production command. Do not rely on whatever
`solana config get` happens to show.

## Deploy or upgrade

For production:

```bash
solana program deploy \
  --url mainnet-beta \
  --program-id <PRODUCTION_PROGRAM_KEYPAIR_PATH> \
  target/deploy/tradestars_arena.so
```

Then verify:

```bash
solana program show <PRODUCTION_PROGRAM_ID> --url mainnet-beta
```

Record:

- program id
- program data address
- upgrade authority
- last deployed slot
- `.so` SHA256 hash
- git commit

The reported program id must equal the value configured in `Anchor.toml`,
`declare_id!`, the web app env, and relayer env.

## Initialize platform

Run `initialize_platform` exactly once per program id.

Do not initialize until these values are final:

- `arena_operator`
- `treasury_wallet`
- `minting_authority`
- `dispute_window_seconds`
- `settlement_grace_period_seconds`

The signer of `initialize_platform` becomes `authority`.

Recommended production values:

- `dispute_window_seconds`: use a real review window, not devnet values.
  Recommended starting point: `3600` to `86400`.
- `settlement_grace_period_seconds`: enough time for the operator to recover
  from ordinary downtime before users can stale-cancel. Recommended starting
  point: `86400`.

After initialization, derive and verify the two global PDAs:

```bash
# platform_config PDA seed: "config"
# tUSDC mint PDA seed: "tusdc_mint"
```

The initialized `platform_config` must decode to the intended role keys. The
`tUSDC` mint must be a Token-2022 mint with `NonTransferable` enabled and
decimals set to `6`.

Never run `initialize_platform` if the `platform_config` PDA already exists.

## Post-init verification

Before enabling the web app or relayer:

1. Verify `program show` reports the expected upgrade authority.
2. Verify `platform_config` has the intended role keys.
3. Verify `deposits_paused == false` only if the relayer is ready.
4. Verify the `tUSDC` mint PDA exists and is Token-2022.
5. Run a small controlled deposit through `deposit_collateral` on the target
   cluster if this is devnet/staging. For mainnet, use the smallest acceptable
   production test amount and record the Base tx/log index.
6. Verify replay protection by confirming the same `(base_tx_hash, log_index)`
   cannot be processed twice.
7. Verify the web app reads the correct `NEXT_PUBLIC_PROGRAM_ID`.
8. Verify the relayer signs with the configured `minting_authority`.

## Required environment alignment

The same production program id must be configured in:

- Solana program `declare_id!`
- `Anchor.toml` `[programs.mainnet]`
- web app `NEXT_PUBLIC_PROGRAM_ID`
- relayer `SOLANA_PROGRAM_ID` or equivalent
- any admin scripts or cron jobs

The same production RPC cluster must be configured in:

- web app Solana RPC env
- relayer Solana RPC env
- admin scripts
- deployment commands

The relayer must hold the private key for `minting_authority`, not the
`authority` or `upgrade_authority`.

## Operational controls

Use `set_deposits_paused(true)` immediately if:

- Base deposit verification is failing.
- The relayer is minting incorrect amounts.
- Alchemy/webhook input is suspected to be spoofed or duplicated.
- The Base deposit contract has an incident.
- The relayer private key may be compromised.

Use `update_platform_config` only from `authority`. Treat every config update as
a production change requiring review and recorded tx signature.

Do not transfer or rotate `upgrade_authority` without recording:

- old authority
- new authority
- command used
- tx signature
- verification output from `solana program show`

## Release checklist

Before switching production traffic:

- Program id verified on-chain.
- Program data address recorded.
- Upgrade authority verified.
- Platform config initialized once.
- Role keys decoded and checked.
- tUSDC mint PDA verified.
- Web app env updated.
- Relayer env updated.
- Admin scripts/cron env updated.
- Deposits paused/unpaused intentionally.
- Monitoring is active for `deposit_collateral`, `WithdrawRequested`, failed
  relayer mints, and failed payouts.

## Common mistakes to avoid

- Deploying with a generated `target/deploy/*-keypair.json` that does not match
  the intended program id.
- Updating `Anchor.toml` but forgetting `declare_id!`.
- Switching the web app to a new program id before initializing platform config.
- Initializing with the deploy wallet as every role.
- Giving the relayer the platform `authority` key.
- Waiting for the UI to confirm withdrawals instead of using the relayer/indexer
  to observe `WithdrawRequested`.
- Running production commands without `--url mainnet-beta`.
- Reusing devnet dispute/grace windows in production without an explicit
  decision.

