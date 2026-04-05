#!/usr/bin/env bash
[ -z "$WINDOWID" ] && exit 0
printf '{"kind":"x11_generic","focus_id":"%s:%d","outer_id":"","label":"Terminal"}\n' \
  "$WINDOWID" "$$"
