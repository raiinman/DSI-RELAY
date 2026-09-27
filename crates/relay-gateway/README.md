# Local discovery gateway

Start the per-user gateway while `relayd` is running:

```powershell
cargo run -p relay-gateway --bin relay-gateway
```

In another terminal, send a local negotiation request:

```powershell
cargo run -p relay-gateway --bin relay-gateway -- probe
```

For a read-only discovery request, list commands with a bounded prefix or describe one command:

```powershell
cargo run -p relay-gateway --bin relay-gateway -- list registry.
cargo run -p relay-gateway --bin relay-gateway -- describe registry.list
```

`probe` sends `POST /v1/negotiate` with `{"protocol_min":1,"protocol_max":1}` to `127.0.0.1:8765`. `list` and `describe` send authenticated `POST /v1/execute` requests limited to the two registry commands. Each client command loads the bearer token from a current-user DPAPI-protected local credential; the token is never entered on a command line or printed. Press Ctrl+C in the server terminal to stop it and remove the credential file. A crash may leave an encrypted stale file; the next successful start replaces it with a new token.

This gateway exposes only bounded `registry.list` and `registry.describe` discovery. It does not expose project data, writes, public networking, or a ChatGPT connection.
