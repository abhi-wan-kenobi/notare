#!/usr/bin/env bash

. "$(dirname "$0")/bash-guard.sh"

set -euo pipefail

sudo apt update
sudo apt update
# The pipewire-upstream PPA only publishes for LTS codenames. `ubuntu-latest`
# moved to 26.04 ("resolute") on 2026-11-02 and `apt update` then fails with
# "does not have a Release file"; 26.04's stock libpipewire-0.3-dev is new
# enough, so only add the PPA where it exists.
codename="$(lsb_release -cs 2>/dev/null || echo unknown)"
case "$codename" in
  jammy | noble)
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
    ;;
  *)
    echo "setup-linux-others: skipping pipewire PPA on Ubuntu '$codename'"
    ;;
esac
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
