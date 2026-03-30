## Context

pane-roster combines three roles that BeOS split across the registrar and BLaunchRoster, and that NeXT provided as the Services menu:

1. **Service directory** — infrastructure servers (pane-comp, pane-route, pane-store, pane-fs) register on startup. Roster is the directory, not the supervisor. The init system (s6/runit/systemd) supervises infrastructure.

2. **Process supervisor** — desktop apps (shells, editors, user-launched programs) are supervised by roster. Roster launches them, monitors them, restarts on crash, and manages session state.

3. **Service registry** — apps advertise operations they can perform on content types. pane-route queries this registry for multi-match scenarios.

The existing typed views: RosterRegister (infrastructure registration with ServerKind enum) and RosterServiceRegister (operation registration with content_type pattern).

## Goals / Non-Goals

**Goals:**
- Define app signatures and how apps identify themselves
- Define the service directory lifecycle (register, query, disconnect)
- Define process supervision (launch, monitor, restart policy, exit handling)
- Define service registry (register operations, query by content type)
- Define session save/restore model

**Non-Goals:**
- Implementation
- The typed view wire format (already in pane-proto)
- Init system integration details (varies by deployment)

## Decisions

### 1. App signatures: reverse-domain strings

Apps identify themselves with reverse-domain signatures like BeOS: `app.pane.shell`, `app.pane.editor`, `app.pane.browser`. Infrastructure servers use `svc.pane.route`, `svc.pane.store`, etc. The signature is sent at registration time and used for queries and single-launch enforcement.

### 2. Single-launch enforcement optional per signature

Apps can declare single-launch behavior: if an instance with the same signature is already running, the new launch request is redirected to the existing instance (with the route message forwarded). This is opt-in — most apps allow multiple instances.

### 3. Restart policy: crash vs clean exit

Roster distinguishes crash (signal, non-zero exit) from clean exit (zero exit, or close via pane protocol). On crash, roster restarts desktop apps by default (with backoff). On clean exit, the app stays dead. This policy is per-signature, configurable.

### 4. Session state: serialized app list + pane layout

Session save captures: which apps are running (signatures), their pane IDs, the layout tree state (from pane-comp). Session restore launches the apps in order and requests pane-comp to restore the layout. Apps restore their own internal state (from their own settings files).

### 5. Service registry: content_type glob patterns

Operations are registered with a content_type pattern (glob, not regex — simpler for content types). `text/*` matches any text type. `*` matches everything. pane-route queries by content_type and gets back matching operations.

## Risks / Trade-offs

**[Restart loops]** → An app that crashes immediately on launch will restart repeatedly. Mitigation: exponential backoff (1s, 2s, 4s, 8s, max 60s). After N consecutive crashes without staying alive for M seconds, stop restarting and log an error.

**[Session restore ordering]** → Apps may depend on each other (editor needs file manager to provide a path). Mitigation: launch all apps and let them handle missing dependencies gracefully. No dependency ordering in session restore — apps are independent.

## Open Questions

- Should roster track which panes belong to which app? pane-comp knows this (the protocol state tracks connections and their panes), but roster might need it for session restore.
- Should roster provide an API for app-to-app communication beyond routing? (BRoster allowed sending BMessages to other apps by signature.)
