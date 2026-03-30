When running builds or tests that produce long output, always tee to /tmp. The user may want to check progress on a long-running build.

Use: `nix build ... 2>&1 | tee /tmp/pane-build.log | tail -30`
Not: `| tail -30` alone.
