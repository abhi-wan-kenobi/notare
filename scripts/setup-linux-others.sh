#!/usr/bin/env bash

. "$(dirname "$0")/bash-guard.sh"

set -euo pipefail

sudo apt update
# add-apt-repository queries the Launchpad API, which intermittently drops
# connections mid-response and would otherwise fail every Linux CI job.
for attempt in 1 2 3 4; do
  if sudo add-apt-repository -y ppa:pipewire-debian/pipewire-upstream; then
    break
  fi
  if [ "$attempt" -eq 4 ]; then
    echo "add-apt-repository failed after $attempt attempts" >&2
    exit 1
  fi
  sleep $((attempt * 10))
done
sudo apt update
sudo apt-get install -y \
  libgtk-3-dev \
  libgtk-4-dev \
  libasound2-dev \
  libudev-dev \
  libpulse-dev \
  libpipewire-0.3-dev \
  libgraphene-1.0-dev \
  pkg-config \
  patchelf \
  cmake \
  curl \
  libcurl4-openssl-dev

curl -fsSL https://get.pnpm.io/install.sh | sh -
