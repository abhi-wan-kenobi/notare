// Shared, dependency-free parsing for NOTARE_SYNC_AGENT_ADDR ("host:port").
//
// Pulled out of network_p2p.c into its own header so it can be unit-tested
// directly (see crates/cloudsync/tests/ipv6_bracket_host_split.rs) without
// dragging in the rest of network_p2p.c's dependencies (the cloudsync SQLite
// allocator, sockets, the vendored build). It uses only getenv/memcpy/strtol
// from the C standard library.
//
// SYNC-9: fixes the §12/§13.9 carried finding. `resolve_agent_addr` splits
// NOTARE_SYNC_AGENT_ADDR at the LAST ':' so the host substring for a bracketed
// IPv6 literal such as "[::1]:1234" keeps its brackets ("[::1]"), and
// getaddrinfo() rejects that — brackets are a URI/host:port display
// convention, not part of a valid numeric host argument. Fix: after the
// split, strip a matching leading '[' / trailing ']' from the host before
// returning it. The host_cap bounds check (fail closed on an oversized host)
// and the "addr missing" failure behavior are both unchanged.

#ifndef __CLOUDSYNC_AGENT_ADDR_H__
#define __CLOUDSYNC_AGENT_ADDR_H__

#include <stdbool.h>
#include <stddef.h>
#include <stdlib.h>
#include <string.h>

// Parse "host:port" out of `addr` (may be NULL / empty). Split at the last
// ':' — the host is always 127.0.0.1 or an IPv6 literal, never itself
// containing a bare (unbracketed) ':' followed by more host characters.
static bool resolve_agent_addr_from(const char *addr, char *host_out, size_t host_cap, int *port_out) {
    if (!addr || !*addr) return false;

    const char *colon = NULL;
    for (const char *p = addr; *p; p++) {
        if (*p == ':') colon = p;
    }
    if (!colon) return false;

    size_t host_len = (size_t)(colon - addr);
    if (host_len == 0 || host_len >= host_cap) return false;
    memcpy(host_out, addr, host_len);
    host_out[host_len] = '\0';

    // Bracketed IPv6 literal, e.g. "[::1]" -> "::1". Reject "[]" (an empty
    // host) rather than stripping it down to an empty string that would
    // otherwise sail through to getaddrinfo() as a "valid" address (audit
    // finding, SYNC-9: gpt-oss caught the off-by-one edge minimax's own
    // walkthrough got wrong — host_len for "[]" is 2, not 1, so the
    // `host_len >= 2` guard alone does not exclude it).
    if (host_len >= 2 && host_out[0] == '[' && host_out[host_len - 1] == ']') {
        size_t stripped_len = host_len - 2;
        if (stripped_len == 0) return false;
        memmove(host_out, host_out + 1, stripped_len);
        host_out[stripped_len] = '\0';
    }

    char port_buf[16];
    size_t port_len = strlen(colon + 1);
    if (port_len == 0 || port_len >= sizeof(port_buf)) return false;
    memcpy(port_buf, colon + 1, port_len);
    port_buf[port_len] = '\0';

    char *end = NULL;
    long port = strtol(port_buf, &end, 10);
    if (end == port_buf || *end != '\0' || port < 1 || port > 65535) return false;

    *port_out = (int)port;
    return true;
}

static bool resolve_agent_addr(char *host_out, size_t host_cap, int *port_out) {
    return resolve_agent_addr_from(getenv("NOTARE_SYNC_AGENT_ADDR"), host_out, host_cap, port_out);
}

#endif
