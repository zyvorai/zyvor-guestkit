#!/usr/bin/env bash
# Copyright 2026 Zyvor AI Labs · https://zyvor.dev
# SPDX-License-Identifier: Apache-2.0
# Gate: server.json (MCP Registry metadata), Dockerfile.mcp and Cargo.toml agree.
# Usage: scripts/check-mcp-registry.sh [tag]   (tag like v1.2.5, optional)
set -euo pipefail
cd "$(dirname "$0")/.."
fail=0
ref=$(sed -n 's/^version = "\(.*\)"$/\1/p' Cargo.toml | head -1)
sj_ver=$(sed -n 's/^  "version": "\([^"]*\)".*/\1/p' server.json)
sj_tag=$(sed -n 's#.*"identifier": "ghcr.io/zyvorai/guestkit-mcp:\([^"]*\)".*#\1#p' server.json)
for pair in "server.json version:$sj_ver" "server.json package tag:$sj_tag"; do
  if [[ "${pair#*:}" != "$ref" ]]; then
    echo "MISMATCH  ${pair%%:*} '${pair#*:}' != Cargo.toml '$ref'" >&2; fail=1
  fi
done
if [[ -n "${1:-}" && "${1#v}" != "$ref" ]]; then
  echo "MISMATCH  tag '$1' != Cargo.toml '$ref'" >&2; fail=1
fi
name=$(sed -n 's/^  "name": "\([^"]*\)".*/\1/p' server.json)
label=$(sed -n 's/^LABEL io.modelcontextprotocol.server.name="\([^"]*\)"$/\1/p' Dockerfile.mcp)
if [[ -z "$name" || "$name" != "$label" ]]; then
  echo "MISMATCH  Dockerfile.mcp label '$label' != server.json name '$name'" >&2; fail=1
fi
desc=$(sed -n 's/^  "description": "\(.*\)",$/\1/p' server.json)
if [[ ${#desc} -gt 100 ]]; then
  echo "server.json description is ${#desc} chars (registry limit 100)" >&2; fail=1
fi
grep -q '"transport": { "type": "stdio" }' server.json || { echo "server.json: stdio transport missing" >&2; fail=1; }
grep -q 'ENTRYPOINT .*"mcp-serve"' Dockerfile.mcp || { echo "Dockerfile.mcp: ENTRYPOINT must run mcp-serve" >&2; fail=1; }
if [[ $fail -ne 0 ]]; then echo "check-mcp-registry: FAILED (reference ${ref})" >&2; exit 1; fi
echo "check-mcp-registry: OK (${ref})"
