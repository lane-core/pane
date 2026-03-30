# Licensing

Pane uses three licenses, chosen by what the code does and where it runs.

**BSD-3-Clause (protocol):** pane-proto
**BSD-2-Clause (kits):** pane-session, pane-app, pane-ui, pane-text, pane-input, pane-store-client, pane-media, pane-ai, pane-notify
**AGPL-3.0-only (servers):** pane-comp, pane-roster, pane-store, pane-fs, pane-watchdog, pane-dbus

## Why three licenses

The **protocol crate** (pane-proto) is BSD-3-Clause. It defines the wire format — what it means to speak pane. The no-endorsement clause prevents third parties from using the pane name to promote derived works without permission. Otherwise it's as permissive as BSD-2-Clause: anyone can implement the protocol, build compatible software, or fork the types. The protocol carries the project's identity, so the name matters here.

The **kit crates** are BSD-2-Clause. These are libraries that live inside the client process — the developer experience. BSD-2-Clause is permissive with no restrictions beyond attribution and the standard disclaimer. The no-endorsement clause is unnecessary for linked libraries; nobody claims endorsement by using a dependency. Anyone can build proprietary pane applications without disclosing source.

The **server crates** are AGPL-3.0-only. These are independent processes that provide infrastructure over unix sockets — the compositor, the roster, the store. AGPL ensures that modifications to shared infrastructure are contributed back, including in network-service deployments.

## The boundary

A kit is a library linked into your process. A server is a separate process you talk to over a socket. The protocol is the wire-format definition shared by both sides. This three-way distinction — protocol identity, in-process library, independent service — determines the license.

## What this means in practice

A third party building a proprietary application with pane kits can do so freely. BSD-2-Clause requires only attribution.

Someone who modifies the compositor and deploys it must share the modified source under AGPL-3.0.

A company using pane-ai to build proprietary agent infrastructure can do so — the kit is a library, not a service.

No one can market a product as "pane-certified" or "powered by pane" without permission. The BSD-3-Clause clause on the protocol handles this at the license level.
