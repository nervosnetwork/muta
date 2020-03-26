#!/bin/bash
set -ev

if [ "$FMT" = true ]; then
  make fmt
fi
if [ "$CHECK" = true ]; then
  make check
  make clippy
fi
if [ "$TEST" = true ]; then
  make test
fi
if [ "$E2E" = true ]; then
  make e2e-test
fi

git diff --exit-code Cargo.lock
