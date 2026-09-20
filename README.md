# xmrtui

Vim-like TUI for a Monero wallet. It does **not** reimplement the wallet: it
spawns official `monero-wallet-rpc` (same engine as `monero-wallet-cli`) and
talks JSON-RPC.

## Requirements

- Rust 1.93+ (`rustup`)
- `monero-wallet-rpc` in `PATH` (`brew install monero` on macOS)

## Build

```bash
git clone git@github.com:ch4-Zz/xmrtui.git
cd xmrtui
cargo build --release
```

The binary is `target/release/xmrtui`. Optional install:

```bash
install -m 755 target/release/xmrtui /usr/local/bin/xmrtui
```

## Usage

```bash
xmrtui --wallet-file ~/Monero/wallets/wallet_1
```

From the build tree, without installing:

```bash
./target/release/xmrtui --wallet-file ~/Monero/wallets/wallet_1
```

Remote node defaults to `node.sethforprivacy.com:18089`, or `127.0.0.1:18081`
if a local `monerod` is already listening. The TUI polls every 5s and shows
**OUT OF SYNC** if the daemon is unreachable.

```bash
xmrtui --wallet-file ~/Monero/wallets/wallet_1 \
  --daemon-address node.monerodevs.org:18089
```

Attach to a wallet RPC you already started:

```bash
xmrtui --rpc-url http://127.0.0.1:18083
```

Password is typed in the TUI, never passed on the command line.

## Keys

| Key | Action |
|---|---|
| `1` `2` `3` | dashboard / transfers / send |
| `gt` / `gT` | next / previous tab |
| `j` `k` `gg` `G` | move in the transfer list |
| `r` | refresh |
| `yy` | yank address (or txid on transfers) |
| `i` | insert mode on the send form |
| `:refresh` | rescan |
| `:send <addr> <amount>` | transfer (asks `y/n`) |
| `:q` | quit |
| `Tab` / `S-Tab` (in `:`) | complete / cycle commands |
