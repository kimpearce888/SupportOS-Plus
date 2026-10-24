#!/bin/bash
# Start (or reuse) the Xvfb virtual display used by the audit (audit artifact, NOT app code).
set -u
if ! pgrep -f "Xvfb :99" >/dev/null 2>&1; then
  (Xvfb :99 -screen 0 1600x1000x24 -nolisten tcp >/tmp/xvfb.log 2>&1 &)
  sleep 2
fi
pgrep -f "Xvfb :99" >/dev/null 2>&1 && echo "Xvfb :99 running" || { echo "Xvfb FAILED to start"; exit 1; }
