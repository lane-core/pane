## Why

pane-roster is the service directory, desktop app supervisor, and operation registry — the BeOS BRoster + BLaunchRoster + NeXT Services synthesis. The architecture spec defines three roles (service directory for infrastructure, process supervisor for apps, operation registry for services) but has no behavioral contracts for app signatures, launch protocols, restart policies, session persistence, or the service discovery model.

## What Changes

- Define the app signature model (how apps identify themselves)
- Define the service directory protocol (how infrastructure servers register)
- Define the process supervisor behavior (how desktop apps are launched, monitored, restarted)
- Define the service registry (how apps advertise operations)
- Define session save/restore
- Define the interaction with pane-route (service query for multi-match)

## Specs Affected

### New
- `pane-roster`: App signatures, service directory, process supervision, service registry, session management

### Modified
- None

## Impact

- New spec at openspec/specs/pane-roster/spec.md
- No code changes — spec only
- RosterRegister and RosterServiceRegister typed views already exist in pane-proto
