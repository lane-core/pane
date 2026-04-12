---
type: reference
status: current
created: 2026-04-11
last_updated: 2026-04-11
importance: high
keywords: [link_protocol, handshake, wire_format, versioning, AS_CREATE_APP, AS_GET_DESKTOP, BMessage, postcard]
sources: [haiku/src/kits/app/link_message.h, haiku/headers/private/app/ServerProtocol.h, haiku/src/kits/app/AppMisc.cpp, haiku/src/servers/app/AppServer.cpp, haiku/src/servers/app/Desktop.cpp, haiku/src/kits/app/Application.cpp, haiku/src/kits/app/LinkSender.cpp, haiku/src/kits/app/LinkReceiver.cpp]
verified_against: [haiku source as of 2026-04-11]
agents: [be-systems-engineer]
---

# Haiku link protocol and handshake wire format analysis

Investigated for pane wire format extensibility question.

## Two-phase handshake discovery

Haiku's app_server connection is a **two-phase handshake** using
**two different wire formats**:

### Phase 1: AS_GET_DESKTOP (self-describing BMessage)

`create_desktop_connection()` in `AppMisc.cpp:210-237`:
- Creates a BMessage(AS_GET_DESKTOP)
- Adds named fields: "user" (int32), "version" (int32 = AS_PROTOCOL_VERSION), "target" (string)
- Sends via BMessenger to "application/x-vnd.Haiku-app_server"
- Gets BMessage reply with "port" field

The server side (`AppServer::MessageReceived`, AppServer.cpp:109-142):
- Checks `version != AS_PROTOCOL_VERSION` — hard reject, logs error
- No version negotiation — exact match or fail
- Returns desktop port on success, B_ERROR on failure

**Key insight: the initial negotiation uses BMessage, which IS
self-describing (named fields, typed values). The protocol
version field is embedded in a format that can always be parsed
regardless of version.**

### Phase 2: AS_CREATE_APP (positional link protocol)

After getting the desktop port, `BApplication::_ConnectToServer()`
(Application.cpp:1402-1436):
- Uses LinkSender::StartMessage(AS_CREATE_APP)
- Attaches fields positionally: port_id, port_id, team_id, int32, char*
- Gets positional reply: port_id, area_id, team_id

This is the raw `link_message.h` protocol:
```c
struct message_header {
    int32  size;    // message size
    uint32 code;    // opcode (AS_CREATE_APP, etc.)
    uint32 flags;   // kNeedsReply = 0x01
};
```

After the header, payload is raw positional bytes. No field names,
no type tags, no length prefixes (except for strings which get
int32 length prefix). Format defined entirely by the opcode.

## Wire versioning story

AS_PROTOCOL_VERSION is defined as 1 in ServerProtocol.h:27.
It has **never been incremented** in Haiku's history. The protocol
has changed by adding new opcodes to the enum (AS_LAST_CODE grows)
rather than changing existing message layouts.

This means: **Haiku has never actually tested their version
mechanism.** There is no negotiation — it's a hard equality check.
Old client + new server = crash/rejection. Old messages with new
fields would break silently because the positional format has no
way to detect extra data.

## BMessage flattened format

BMessage IS self-describing when flattened:
- Named fields with type codes
- Field count and sizes in header
- `MESSAGE_FORMAT_HAIKU` magic at start
- `MessageAdapter::Unflatten` handles older formats

But BMessage was NOT used for the high-frequency link protocol.
It was used for the initial handshake (AS_GET_DESKTOP), clipboard,
drag-and-drop, inter-app messaging — places where flexibility
mattered more than speed.

## Implications for pane

The Haiku architecture implicitly validates pane's option B
("self-describing handshake, binary data plane"):

1. Haiku already did this — BMessage for AS_GET_DESKTOP, link
   protocol for everything after
2. The link protocol's lack of extensibility was never fixed;
   AS_PROTOCOL_VERSION=1 forever. Changes required recompilation
   of both sides.
3. BMessage's self-describing format was the escape valve for
   anything that needed to cross version boundaries (clipboard,
   replicants, drag-and-drop, archived views)

The positional link protocol worked because BeOS/Haiku controlled
both sides — app_server and libroot shipped together, always in
lockstep. Pane cannot assume this (different pane versions on
same system, network connections, etc.).
