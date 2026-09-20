# xmrtui

Vim-like TUI for a Monero wallet. It does **not** reimplement the wallet: it
spawns official `monero-wallet-rpc` (same engine as `monero-wallet-cli`) and
talks JSON-RPC.

## Run

```bash
brew install monero   # provides monero-wallet-rpc
cargo run --release -- --wallet-file ~/Monero/wallets/wallet_1
```

Remote node (default): `node.moneroworld.com:18089`.

```bash
cargo run --release -- \
  --wallet-file ~/Monero/wallets/wallet_1 \
  --daemon-address other.node:18089
```

Attach to an RPC you already started:

```bash
cargo run --release -- --rpc-url http://127.0.0.1:18083
```

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

Password is typed in the TUI, never passed on the command line.
