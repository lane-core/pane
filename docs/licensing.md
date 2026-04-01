# Licensing

Pane uses two licenses, chosen by what the code does and where it runs.

**BSD-3-Clause (protocol + kits):** pane-proto, pane-session, pane-app, pane-optic, pane-notify, pane-hello
**AGPL-3.0-only (servers):** pane-comp, pane-headless, pane-server

Future crates follow the same split: kits (client libraries) are BSD-3-Clause, servers (infrastructure processes) are AGPL-3.0-only.

## Why two licenses

The **kit crates** are BSD-3-Clause. These are libraries linked into the client process — the developer experience. BSD-3-Clause is permissive with attribution and a no-endorsement clause. Anyone can build proprietary pane applications without disclosing source.

The **server crates** are AGPL-3.0-only. These are independent processes that provide infrastructure over unix sockets — the compositor, the headless server, the protocol server library. AGPL ensures that modifications to shared infrastructure are contributed back, including in network-service deployments.

## The boundary

A kit is a library linked into your process. A server is a separate process you talk to over a socket. This distinction — in-process library vs independent service — determines the license.

## What this means in practice

A third party building a proprietary application with pane kits can do so freely. BSD-3-Clause requires only attribution and the no-endorsement clause.

Someone who modifies the compositor or headless server and deploys it must share the modified source under AGPL-3.0.

No one can market a product as "pane-certified" or "powered by pane" without permission — the BSD-3-Clause no-endorsement clause handles this.
