# Demo 01 — Deploy the Contract

> **Contract version:** `SCHEMA_VERSION = 6`  
> **Estimated recording length:** < 5 minutes  
> **Prerequisites:** Rust stable, `wasm32v1-none` target, Stellar CLI v22+, Docker

This demo shows how to go from source code to a deployed escrow contract on a
local Soroban validator. By the end you will have a live `CONTRACT_ID` and a
test token ready for the subsequent demos.

---

## Recording script

Use this script as a cue sheet when recording. Narration cues are in
_italics_; shell commands are in code blocks.

---

### Part 1 — Build the WASM (0:00 – 0:45)

_"We start from a clean checkout. The first thing to do is make sure the
WASM target is present and then do a release build."_

```bash
rustup target add wasm32v1-none
```

_"That's idempotent — if the target is already installed it's a no-op."_

```bash
cargo build --target wasm32v1-none --release -p karis-ky_escrow
```

_"The release build produces an optimised WASM artifact."_

```bash
ls -lh target/wasm32v1-none/release/karis-ky_escrow.wasm
```

**Expected output:**
```
-rw-r--r-- 1 user user 143K Jul 25 09:00 target/wasm32v1-none/release/karis-ky_escrow.wasm
```

_"Good. We have a WASM file. The exact size will vary with the release."_

---

### Part 2 — Start the local standalone validator (0:45 – 1:30)

_"Next we start a local Soroban-enabled validator in Docker. This gives us a
private network with no real funds."_

```bash
stellar container start local
```

_"Wait for the container to report it's ready — usually about 10 seconds."_

```bash
# Register it as a named network so all subsequent commands use --network local
stellar network add \
  --rpc-url http://localhost:8000/soroban/rpc \
  --network-passphrase "Standalone Network ; February 2017" \
  local
```

_"Confirm connectivity:"_

```bash
stellar network ls
```

**Expected output (abbreviated):**
```
NAME     RPC URL                                PASSPHRASE
local    http://localhost:8000/soroban/rpc      Standalone Network ; February 2017
```

---

### Part 3 — Create identities (1:30 – 2:15)

_"We need five named keypairs: admin, SME, two investors, and treasury. The
`--fund` flag tops them up from the local friendbot."_

```bash
stellar keys generate admin     --network local --fund
stellar keys generate sme       --network local --fund
stellar keys generate investor1 --network local --fund
stellar keys generate investor2 --network local --fund
stellar keys generate treasury  --network local --fund
```

_"Save the addresses as shell variables for everything that follows."_

```bash
ADMIN=$(stellar keys address admin)
SME=$(stellar keys address sme)
INVESTOR1=$(stellar keys address investor1)
INVESTOR2=$(stellar keys address investor2)
TREASURY=$(stellar keys address treasury)

echo "ADMIN:     $ADMIN"
echo "SME:       $SME"
echo "INVESTOR1: $INVESTOR1"
echo "INVESTOR2: $INVESTOR2"
echo "TREASURY:  $TREASURY"
```

**Expected output (addresses will differ):**
```
ADMIN:     GADMINXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX
SME:       GSMEXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX
INVESTOR1: GINV1XXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX
INVESTOR2: GINV2XXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX
TREASURY:  GTREAXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX
```

> These are simulation-only throwaway keys. Never commit real secret keys to
> version control or log them.

---

### Part 4 — Deploy a test token (2:15 – 2:50)

_"The escrow contract needs a SEP-41 token. For local simulation we wrap the
native XLM asset."_

```bash
TOKEN_ID=$(stellar contract asset deploy \
  --asset native \
  --source admin \
  --network local)

echo "TOKEN_ID: $TOKEN_ID"
```

**Expected output:**
```
TOKEN_ID: CAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAABBSC4
```

_"We now have a token contract address. In production this would be a
properly audited stablecoin like USDC; for the demo the native wrapper is
sufficient."_

---

### Part 5 — Deploy the escrow contract (2:50 – 3:45)

_"Finally, we deploy the escrow WASM. The `admin` identity pays the fee and
owns the contract instance."_

```bash
CONTRACT_ID=$(stellar contract deploy \
  --wasm target/wasm32v1-none/release/karis-ky_escrow.wasm \
  --source admin \
  --network local)

echo "CONTRACT_ID: $CONTRACT_ID"
```

**Expected output:**
```
CONTRACT_ID: CBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBSC4
```

_"The deploy command uploads the WASM to the network and creates a contract
instance. The returned address is the contract ID we'll use for every
subsequent `stellar contract invoke` call."_

---

### Part 6 — Verify the deployment (3:45 – 4:15)

_"The escrow contract is deployed but not yet initialised — `init` hasn't
been called. We can confirm this by asking for the version stored on-chain,
which will return an error since no state has been written yet."_

```bash
# This is expected to return an error — the contract is not yet initialised
stellar contract invoke \
  --id "$CONTRACT_ID" \
  --network local \
  -- get_version 2>&1 || echo "[expected: contract not yet initialised]"
```

_"That error is correct. In Demo 02 we call `init` and then `get_version`
returns `6`."_

---

### Part 7 — Export for subsequent demos (4:15 – 4:30)

_"Before wrapping up, let's show the variables that carry forward."_

```bash
echo "export CONTRACT_ID=$CONTRACT_ID"
echo "export TOKEN_ID=$TOKEN_ID"
echo "export ADMIN=$ADMIN"
echo "export SME=$SME"
echo "export INVESTOR1=$INVESTOR1"
echo "export INVESTOR2=$INVESTOR2"
echo "export TREASURY=$TREASURY"
```

_"Copy these exports into your shell session before running Demo 02."_

---

## Transcript summary

| Step | What happened |
|------|---------------|
| Build | `cargo build --target wasm32v1-none --release` produced `karis-ky_escrow.wasm` |
| Start node | `stellar container start local` launched a Soroban standalone validator |
| Identities | Five named keypairs created and funded from the local friendbot |
| Token | Native XLM wrapped as a SEP-41 token via `stellar contract asset deploy` |
| Deploy | Escrow WASM uploaded and a contract instance created; `CONTRACT_ID` captured |
| Verify | `get_version` returned an error as expected — contract awaits `init` |

---

## Troubleshooting

**`stellar container start local` fails with "port already in use"**  
Stop any existing container: `stellar container stop local`, then retry.

**`cargo build` fails with `error[E0463]: can't find crate`**  
Run `cargo update` then retry the build.

**`stellar contract deploy` returns `HostError`**  
Confirm the local node is running: `stellar network ls` and check Docker is
running. Re-run `stellar container start local` if needed.

**All `stellar` commands fail with "command not found"**  
The Stellar CLI is not on your `PATH`. Run:
```bash
cargo install --locked stellar-cli --features opt
```
Then open a new terminal session.

---

## Next

Continue to [Demo 02 — Initialize an Escrow](02-initialize-escrow.md).
