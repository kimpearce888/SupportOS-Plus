#!/bin/bash
# Reusable launch helper for SupportOS-Plus QA on this sandbox (audit artifact, NOT app code).
# Usage: ./run-app.sh <seconds-to-keep-running-in-foreground-call>   (omit arg = keep in background of the call)
# Requires: Xvfb started on :99 (see start-xvfb.sh), built binary at target/debug/supportos-plus
set -u
export DISPLAY=:99
export LD_LIBRARY_PATH=/home/z/.local/audit-libs/usr/lib/x86_64-linux-gnu${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}
export LIBGL_ALWAYS_SOFTWARE=1
cd /home/z/my-project/audit/SupportOS-Plus
exec ./target/debug/supportos-plus
