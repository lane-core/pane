# Haiku Source Reference

Curated subset of Haiku source code included as design reference for
pane's Application Kit ancestry. 167 files, ~65K LOC.

Haiku is copyright Haiku, Inc. and contributors, released under the
MIT license. Full source: https://github.com/haiku/haiku

## What's here

The subsystems pane descends from directly. Drawing, font, bitmap,
and widget implementations are excluded — pane uses Smithay/Wayland.

    src/kits/app/           — Application Kit (BApplication, BLooper,
                              BHandler, BMessenger, BMessage, etc.)
    src/kits/interface/     — Window.cpp, View.cpp only (threading model)
    src/kits/support/       — BLocker, BArchivable, BDataIO
    src/kits/storage/       — BQuery, BNode, NodeMonitor (pane-fs ancestry)
    src/servers/app/        — app_server core (ServerApp, ServerWindow,
                              Desktop, EventDispatcher, decorators)
    src/servers/registrar/  — TRoster, Clipboard, MessageDeliverer
    src/servers/launch/     — launch_daemon (service management)
    headers/os/             — public API headers
    headers/private/        — internal headers (wire protocol, internals)

## Key files for pane development

    headers/private/app/ServerProtocol.h    — app_server wire protocol (~370 opcodes)
    headers/private/app/MessagePrivate.h    — BMessage internals (field layout, reply chain)
    src/kits/app/Looper.cpp                — BLooper message loop, Lock/Unlock
    src/kits/app/Application.cpp           — BApplication, app_server connection
    src/kits/app/LinkSender.cpp            — binary protocol batching/flushing
    src/kits/app/LinkReceiver.cpp          — binary protocol deserialization
    src/kits/interface/Window.cpp          — per-window thread model
    src/servers/app/ServerApp.cpp           — server-side per-app handler
    src/servers/app/ServerWindow.cpp        — server-side per-window handler
    src/servers/registrar/TRoster.cpp       — roster implementation

## Related serena memories

    pane/beapi_divergences      — public API mapping table
    pane/beapi_translation_rules — 8 translation rules
    pane/beapi_internals        — internal mechanism knowledge (wire protocol,
                                  lock contention, BMessage reply, clipboard,
                                  roster, error handling, threading model)
