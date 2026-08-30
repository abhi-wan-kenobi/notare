// P2P network layer for the vendored sqlite-sync (CloudSync) extension.
//
// Replaces the S0b network_stub.c behind the `from-source` feature. Implements
// the two functions the CloudSync core calls (see docs/internal/sync-p2p.md):
//
//   bool          network_send_buffer(network_data *data, const char *endpoint,
//                                     const char *authentication, const void *blob, int blob_size);
//   NETWORK_RESULT network_receive_buffer(network_data *data, const char *endpoint,
//                                     const char *authentication, bool zero_terminated,
//                                     bool is_post_request, char *json_payload,
//                                     const char *custom_header);
//
// Transport: the C layer is deliberately dumb and LOCAL. It does NOT speak
// QUIC/iroh and does NOT dial peers by node id. It opens a plain POSIX TCP
// socket to the in-process Rust P2pAgent (crates/sync-p2p/src/agent.rs) on
// 127.0.0.1:<port> and sends the SAME framed length-prefixed JSON the TCP
// spike used — the full endpoint URL (p2p://<node-id-fingerprint>/... or
// mem://<node-id-fingerprint>/...) travels inside the frame. The agent owns
// the iroh endpoint, enforces the peer allowlist at dial AND accept, and
// relays each request to the addressed peer over an iroh bi-stream. This
// keeps the C transport dead simple (one local socket, no crypto, no async
// runtime) and quarantines the entire rustls/quinn dependency tree inside
// the Rust process.
//
// The agent's local TCP address is supplied by the host process via the
// NOTARE_SYNC_AGENT_ADDR env var ("127.0.0.1:<port>"), set when the agent
// starts. No libcurl, no external deps. Blocking only (contract §6).
//
// Audit status: the §12 SSRF finding is closed by the allowlist enforcement
// in the Rust agent (refused at dial + accept), not in this C layer — the C
// layer only ever talks to the local agent, never to an arbitrary host.

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>
#include <errno.h>
#include <sys/types.h>
#include <sys/socket.h>
#include <netinet/in.h>
#include <arpa/inet.h>
#include <netdb.h>

#include "network_private.h"

// Use the CloudSync SQLite allocator so returned buffers are freed by the
// core's network_result_cleanup() via cloudsync_memory_free() (= sqlite3_free).
// cloudsync_memory_zeroalloc / cloudsync_memory_free resolve (through utils.h)
// to dbmem_* → sqlite3_malloc / sqlite3_free for the extension build.
#include "../utils.h"

// ------------------------------------------------------------------
// local agent address resolution
// ------------------------------------------------------------------

// The C network layer is deliberately dumb and LOCAL: it does NOT speak
// QUIC/iroh and does NOT dial peers by node id. It opens a plain TCP socket
// to the in-process Rust P2pAgent (crates/sync-p2p/src/agent.rs) on
// 127.0.0.1:<port> and sends the SAME framed length-prefixed JSON the TCP
// spike used — the full endpoint URL (p2p://<node-id-fingerprint>/... or
// mem://<node-id-fingerprint>/...) travels inside the frame, and the agent
// does the iroh routing + allowlist enforcement. This quarantines the entire
// rustls/quinn dep tree inside the Rust process.
//
// The agent's local TCP address is supplied by the host process via the
// NOTARE_SYNC_AGENT_ADDR env var (set to 127.0.0.1:<port> when the agent
// starts). Reading it once per call is cheap and avoids a global init
// ordering dependency on the extension load path.

static bool resolve_agent_addr(char *host_out, size_t host_cap, int *port_out) {
    const char *addr = getenv("NOTARE_SYNC_AGENT_ADDR");
    if (!addr || !*addr) return false;
    // Parse "host:port". Split at the last ':' (the host is 127.0.0.1 / ::1).
    const char *colon = NULL;
    for (const char *p = addr; *p; p++) {
        if (*p == ':') colon = p;
    }
    if (!colon) return false;
    size_t host_len = (size_t)(colon - addr);
    if (host_len == 0 || host_len >= host_cap) return false;
    memcpy(host_out, addr, host_len);
    host_out[host_len] = '\0';

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

// SYNC-5: the bearer token for the C↔agent socket, read from the
// NOTARE_SYNC_TOKEN env var on every network call (same process-local,
// read-per-call model as NOTARE_SYNC_AGENT_ADDR above). The agent mints the
// token at start and rejects any frame whose token does not match, closing the
// §14 audit finding (any local process that can reach the port could otherwise
// read/write sync data). The token is process-local — it is NOT sent to the
// remote peer over iroh; the inbound iroh path is gated by the Ed25519-
// authenticated EndpointId + allowlist, not by this token.
static bool resolve_agent_token(char *out, size_t cap) {
    const char *tok = getenv("NOTARE_SYNC_TOKEN");
    if (!tok || !*tok) return false;
    size_t len = strlen(tok);
    if (len == 0 || len >= cap) return false;
    memcpy(out, tok, len);
    out[len] = '\0';
    return true;
}

// ------------------------------------------------------------------
// framed TCP framing (4-byte big-endian length + JSON payload)
// ------------------------------------------------------------------

static bool write_all(int fd, const void *buf, size_t len) {
    const char *p = (const char *)buf;
    while (len > 0) {
        ssize_t n = send(fd, p, len, 0);
        if (n <= 0) {
            if (n < 0 && errno == EINTR) continue;
            return false;
        }
        p += n;
        len -= (size_t)n;
    }
    return true;
}

static bool read_all(int fd, void *buf, size_t len) {
    char *p = (char *)buf;
    while (len > 0) {
        ssize_t n = recv(fd, p, len, 0);
        if (n <= 0) {
            if (n < 0 && errno == EINTR) continue;
            return false;
        }
        p += n;
        len -= (size_t)n;
    }
    return true;
}

static bool write_frame(int fd, const void *json, size_t len) {
    uint8_t hdr[4];
    hdr[0] = (uint8_t)((len >> 24) & 0xff);
    hdr[1] = (uint8_t)((len >> 16) & 0xff);
    hdr[2] = (uint8_t)((len >> 8) & 0xff);
    hdr[3] = (uint8_t)(len & 0xff);
    return write_all(fd, hdr, 4) && write_all(fd, json, len);
}

// Read one frame into a cloudsync_memory_zeroalloc'd buffer (NUL-terminated).
// Returns the buffer (caller owns via cloudsync_memory_free) or NULL.
static char *read_frame(int fd, size_t *out_len) {
    uint8_t hdr[4];
    if (!read_all(fd, hdr, 4)) return NULL;
    size_t len = ((size_t)hdr[0] << 24) | ((size_t)hdr[1] << 16) |
                ((size_t)hdr[2] << 8) | (size_t)hdr[3];
    if (len > 64 * 1024 * 1024) return NULL;
    char *buf = (char *)cloudsync_memory_zeroalloc(len + 1);
    if (!buf) return NULL;
    if (!read_all(fd, buf, len)) {
        cloudsync_memory_free(buf);
        return NULL;
    }
    buf[len] = '\0';
    if (out_len) *out_len = len;
    return buf;
}

static int connect_tcp(const char *host, int port) {
    char port_str[16];
    snprintf(port_str, sizeof(port_str), "%d", port);

    struct addrinfo hints, *res = NULL, *rp;
    memset(&hints, 0, sizeof(hints));
    hints.ai_family = AF_UNSPEC;
    hints.ai_socktype = SOCK_STREAM;
    if (getaddrinfo(host, port_str, &hints, &res) != 0 || !res) return -1;

    int fd = -1;
    for (rp = res; rp; rp = rp->ai_next) {
        fd = socket(rp->ai_family, rp->ai_socktype, rp->ai_protocol);
        if (fd < 0) continue;
        if (connect(fd, rp->ai_addr, rp->ai_addrlen) == 0) break;
        close(fd);
        fd = -1;
    }
    freeaddrinfo(res);
    return fd;
}

// ------------------------------------------------------------------
// minimal JSON string escaping for POST bodies
// ------------------------------------------------------------------

// The POST bodies the core hands us (json_payload) are already valid JSON
// (`{"dbVersion":N,"seq":S}` / `{"url":"...","dbVersionMin":...,"dbVersionMax":...}`).
// We base64-encode them into the `body` field of our Request frame so the
// broker's `body: Option<Vec<u8>>` (base64) serde field decodes them back to the
// raw JSON bytes. The Request frame is:
//   {"endpoint":"<escaped endpoint>","is_post":<true|false>,"body":"<b64 or null>"}

static char *json_escape(const char *s) {
    if (!s) return NULL;
    size_t cap = strlen(s) * 6 + 1;
    char *out = (char *)cloudsync_memory_zeroalloc(cap);
    if (!out) return NULL;
    size_t j = 0;
    for (const char *p = s; *p; p++) {
        unsigned char c = (unsigned char)*p;
        if (c == '\\' || c == '"') { out[j++] = '\\'; out[j++] = (char)c; }
        else if (c == '\n') { out[j++] = '\\'; out[j++] = 'n'; }
        else if (c == '\r') { out[j++] = '\\'; out[j++] = 'r'; }
        else if (c == '\t') { out[j++] = '\\'; out[j++] = 't'; }
        else if (c < 0x20) { j += snprintf(out + j, cap - j, "\\u%04x", c); }
        else out[j++] = (char)c;
    }
    out[j] = '\0';
    return out;
}

// base64-encode `len` bytes into a cloudsync-allocated NUL-terminated string.
static char *base64_encode(const void *data, size_t len) {
    static const char b64[] = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    const uint8_t *in = (const uint8_t *)data;
    size_t enc_len = ((len + 2) / 3) * 4;
    char *out = (char *)cloudsync_memory_zeroalloc(enc_len + 1);
    if (!out) return NULL;
    size_t k = 0;
    for (size_t i = 0; i < len; i += 3) {
        uint32_t v = (uint32_t)in[i] << 16;
        if (i + 1 < len) v |= (uint32_t)in[i + 1] << 8;
        if (i + 2 < len) v |= (uint32_t)in[i + 2];
        out[k++] = b64[(v >> 18) & 0x3f];
        out[k++] = b64[(v >> 12) & 0x3f];
        out[k++] = (i + 1 < len) ? b64[(v >> 6) & 0x3f] : '=';
        out[k++] = (i + 2 < len) ? b64[v & 0x3f] : '=';
    }
    out[k] = '\0';
    return out;
}

// Build a Request frame for network_receive_buffer. Returns a malloc'd frame
// string (cloudsync allocator) and its length. Caller frees. The `body` is
// base64-encoded so the broker's `Option<Vec<u8>>` (base64) field decodes it.
// `token` is the SYNC-5 bearer token carried in every frame; it is process-local
// and never sent over the iroh peer link.
static char *build_request_frame(const char *endpoint, bool is_post,
                                 const char *json_payload, const char *token,
                                 size_t *out_len) {
    char *ep_esc = json_escape(endpoint);
    if (!ep_esc) return NULL;

    // base64-encode the POST body (the core's json_payload, already valid JSON)
    // so the broker's `body: Option<Vec<u8>>` base64 field decodes it back to
    // the raw JSON bytes. GET → null.
    char *body_b64 = NULL;
    if (is_post && json_payload) {
        body_b64 = base64_encode(json_payload, strlen(json_payload));
        if (!body_b64) { cloudsync_memory_free(ep_esc); return NULL; }
    }

    char *tok_esc = json_escape(token ? token : "");
    if (!tok_esc) { cloudsync_memory_free(ep_esc); cloudsync_memory_free(body_b64); return NULL; }

    const char *body_json = body_b64 ? body_b64 : "null";
    const char *body_quoted = body_b64 ? "\"" : "";
    const char *body_close = body_b64 ? "\"" : "";
    size_t cap = strlen(ep_esc) + strlen(body_json) + strlen(tok_esc) + 64;
    char *frame = (char *)cloudsync_memory_zeroalloc(cap);
    if (!frame) { cloudsync_memory_free(ep_esc); cloudsync_memory_free(body_b64); cloudsync_memory_free(tok_esc); return NULL; }

    int n = snprintf(frame, cap,
        "{\"token\":\"%s\",\"endpoint\":\"%s\",\"is_post\":%s,\"body\":%s%s%s}",
        tok_esc, ep_esc, is_post ? "true" : "false", body_quoted, body_json, body_close);
    cloudsync_memory_free(ep_esc);
    cloudsync_memory_free(body_b64);
    cloudsync_memory_free(tok_esc);
    if (n < 0 || (size_t)n >= cap) { cloudsync_memory_free(frame); return NULL; }
    if (out_len) *out_len = (size_t)n;
    return frame;
}

// Build a PutRequest frame for network_send_buffer:
//   {"token":"<token>","url":"<escaped endpoint>","blob":"<base64 blob>"}
static char *build_put_frame(const char *endpoint, const void *blob, int blob_size,
                             const char *token, size_t *out_len) {
    char *ep_esc = json_escape(endpoint);
    if (!ep_esc) return NULL;

    char *b64buf = base64_encode(blob, (size_t)blob_size);
    if (!b64buf) { cloudsync_memory_free(ep_esc); return NULL; }

    char *tok_esc = json_escape(token ? token : "");
    if (!tok_esc) { cloudsync_memory_free(ep_esc); cloudsync_memory_free(b64buf); return NULL; }

    size_t cap = strlen(ep_esc) + strlen(b64buf) + strlen(tok_esc) + 48;
    char *frame = (char *)cloudsync_memory_zeroalloc(cap);
    if (!frame) { cloudsync_memory_free(ep_esc); cloudsync_memory_free(b64buf); cloudsync_memory_free(tok_esc); return NULL; }

    int n = snprintf(frame, cap, "{\"token\":\"%s\",\"url\":\"%s\",\"blob\":\"%s\"}",
                      tok_esc, ep_esc, b64buf);
    cloudsync_memory_free(ep_esc);
    cloudsync_memory_free(b64buf);
    cloudsync_memory_free(tok_esc);
    if (n < 0 || (size_t)n >= cap) { cloudsync_memory_free(frame); return NULL; }
    if (out_len) *out_len = (size_t)n;
    return frame;
}

// ------------------------------------------------------------------
// the two CloudSync network functions
// ------------------------------------------------------------------

NETWORK_RESULT network_receive_buffer(network_data *data, const char *endpoint,
                                       const char *authentication, bool zero_terminated,
                                       bool is_post_request, char *json_payload,
                                       const char *custom_header) {
    (void)data;
    (void)authentication;   // spike: no auth; broker trusts localhost
    (void)custom_header;   // non-HTTP transport; HTTP headers irrelevant

    if (!endpoint) {
        return (NETWORK_RESULT){CLOUDSYNC_NETWORK_ERROR, NULL, 0, NULL, NULL};
    }

    char host[256];
    int port;
    // The endpoint URL (p2p://<node-id>/... or mem://<node-id>/...) is opaque
    // to the C layer — it travels inside the frame for the Rust agent to
    // route. The C layer only needs the LOCAL agent's TCP address.
    if (!resolve_agent_addr(host, sizeof(host), &port)) {
        char *msg = cloudsync_string_dup(
            "network_receive_buffer: NOTARE_SYNC_AGENT_ADDR not set");
        return (NETWORK_RESULT){CLOUDSYNC_NETWORK_ERROR, msg, 1, NULL, NULL};
    }

    // SYNC-5: the bearer token for the C↔agent socket. Without it the agent
    // rejects the frame with a 401, so a missing token fails the call with a
    // clear error — same style as the addr-not-set path above.
    char token[256];
    if (!resolve_agent_token(token, sizeof(token))) {
        char *msg = cloudsync_string_dup(
            "network_receive_buffer: NOTARE_SYNC_TOKEN not set");
        return (NETWORK_RESULT){CLOUDSYNC_NETWORK_ERROR, msg, 1, NULL, NULL};
    }

    int fd = connect_tcp(host, port);
    if (fd < 0) {
        char *msg = cloudsync_string_dup("network_receive_buffer: connect failed");
        return (NETWORK_RESULT){CLOUDSYNC_NETWORK_ERROR, msg, 1, NULL, NULL};
    }

    size_t frame_len = 0;
    char *frame = build_request_frame(endpoint, is_post_request, json_payload, token, &frame_len);
    if (!frame) {
        close(fd);
        char *msg = cloudsync_string_dup("network_receive_buffer: oom building frame");
        return (NETWORK_RESULT){CLOUDSYNC_NETWORK_ERROR, msg, 1, NULL, NULL};
    }

    bool ok = write_frame(fd, frame, frame_len);
    cloudsync_memory_free(frame);
    if (!ok) {
        close(fd);
        char *msg = cloudsync_string_dup("network_receive_buffer: write failed");
        return (NETWORK_RESULT){CLOUDSYNC_NETWORK_ERROR, msg, 1, NULL, NULL};
    }

    size_t resp_len = 0;
    char *resp = read_frame(fd, &resp_len);
    close(fd);
    if (!resp) {
        char *msg = cloudsync_string_dup("network_receive_buffer: read failed");
        return (NETWORK_RESULT){CLOUDSYNC_NETWORK_ERROR, msg, 1, NULL, NULL};
    }

    // The response frame is JSON: {"status":N,"body":<base64|null>,"error":...}
    // The broker's response shape is stable; do a tolerant manual extraction of
    // the base64 "body" field (jsmn helpers are static in network.c — unavailable
    // here) then base64-decode it.

    // AUDIT (2026-08-27, 2-seat agreement: kimi HIGH #1 + mistral): the status
    // field must gate the body. Without this, a broker error response that
    // carries a body (e.g. {"status":500,"body":"..."}) is handed to the core as
    // a valid sync buffer, and an error with a null body reads as "no changes".
    char *st_key = strstr(resp, "\"status\"");
    if (!st_key) {
        cloudsync_memory_free(resp);
        char *msg = cloudsync_string_dup("network_receive_buffer: no status field");
        return (NETWORK_RESULT){CLOUDSYNC_NETWORK_ERROR, msg, 1, NULL, NULL};
    }
    char *st_val = st_key + 8;
    while (*st_val == ' ' || *st_val == ':' || *st_val == '\t' ||
           *st_val == '\n' || *st_val == '\r') st_val++;
    long status = strtol(st_val, NULL, 10);
    if (status < 200 || status > 299) {
        cloudsync_memory_free(resp);
        char *msg = cloudsync_string_dup("network_receive_buffer: broker returned non-2xx");
        return (NETWORK_RESULT){CLOUDSYNC_NETWORK_ERROR, msg, 1, NULL, NULL};
    }

    // Find "body":"..."  or  "body":null
    char *key = strstr(resp, "\"body\"");
    if (!key) {
        cloudsync_memory_free(resp);
        char *msg = cloudsync_string_dup("network_receive_buffer: no body field");
        return (NETWORK_RESULT){CLOUDSYNC_NETWORK_ERROR, msg, 1, NULL, NULL};
    }
    char *val = key + 6;
    // AUDIT (kimi #5): skip all JSON whitespace, not just space/tab, so a
    // pretty-printed response is not rejected as "bad body value".
    while (*val == ' ' || *val == ':' || *val == '\t' ||
           *val == '\n' || *val == '\r') val++;
    if (strncmp(val, "null", 4) == 0) {
        // 204 No Content: success with no body.
        cloudsync_memory_free(resp);
        return (NETWORK_RESULT){CLOUDSYNC_NETWORK_OK, NULL, 0, NULL, NULL};
    }
    if (*val != '"') {
        cloudsync_memory_free(resp);
        char *msg = cloudsync_string_dup("network_receive_buffer: bad body value");
        return (NETWORK_RESULT){CLOUDSYNC_NETWORK_ERROR, msg, 1, NULL, NULL};
    }

    // val points at the opening quote of a base64 string. Find the closing quote.
    char *b64_start = val + 1;
    char *b64_end = strchr(b64_start, '"');
    if (!b64_end) {
        cloudsync_memory_free(resp);
        char *msg = cloudsync_string_dup("network_receive_buffer: unterminated body");
        return (NETWORK_RESULT){CLOUDSYNC_NETWORK_ERROR, msg, 1, NULL, NULL};
    }
    size_t b64_len = (size_t)(b64_end - b64_start);

    // AUDIT (2026-08-27, 2-seat agreement: gpt-oss + kimi #3): validate the
    // base64 before decoding. Without the %4 check the final iteration reads
    // b64_start[i+1..3] past the closing quote (still inside `resp`, so not a
    // heap overread, but it decodes the surrounding JSON into the payload);
    // without the alphabet check any stray byte silently decodes as 0 instead
    // of erroring.
    if (b64_len == 0 || (b64_len % 4) != 0) {
        cloudsync_memory_free(resp);
        char *msg = cloudsync_string_dup("network_receive_buffer: bad base64 length");
        return (NETWORK_RESULT){CLOUDSYNC_NETWORK_ERROR, msg, 1, NULL, NULL};
    }
    for (size_t i = 0; i < b64_len; i++) {
        char bc = b64_start[i];
        bool valid = (bc >= 'A' && bc <= 'Z') || (bc >= 'a' && bc <= 'z') ||
                     (bc >= '0' && bc <= '9') || bc == '+' || bc == '/' ||
                     (bc == '=' && i >= b64_len - 2);
        if (!valid) {
            cloudsync_memory_free(resp);
            char *msg = cloudsync_string_dup("network_receive_buffer: bad base64 char");
            return (NETWORK_RESULT){CLOUDSYNC_NETWORK_ERROR, msg, 1, NULL, NULL};
        }
    }

    // base64-decode into a cloudsync-allocated, NUL-terminated buffer.
    size_t dec_cap = (b64_len / 4) * 3 + 1;
    char *body = (char *)cloudsync_memory_zeroalloc(dec_cap + 1);
    if (!body) {
        cloudsync_memory_free(resp);
        char *msg = cloudsync_string_dup("network_receive_buffer: oom decoding");
        return (NETWORK_RESULT){CLOUDSYNC_NETWORK_ERROR, msg, 1, NULL, NULL};
    }

    static const int8_t b64tab[256] = {
        ['A']=0,['B']=1,['C']=2,['D']=3,['E']=4,['F']=5,['G']=6,['H']=7,
        ['I']=8,['J']=9,['K']=10,['L']=11,['M']=12,['N']=13,['O']=14,['P']=15,
        ['Q']=16,['R']=17,['S']=18,['T']=19,['U']=20,['V']=21,['W']=22,['X']=23,
        ['Y']=24,['Z']=25,
        ['a']=26,['b']=27,['c']=28,['d']=29,['e']=30,['f']=31,['g']=32,['h']=33,
        ['i']=34,['j']=35,['k']=36,['l']=37,['m']=38,['n']=39,['o']=40,['p']=41,
        ['q']=42,['r']=43,['s']=44,['t']=45,['u']=46,['v']=47,['w']=48,['x']=49,
        ['y']=50,['z']=51,
        ['0']=52,['1']=53,['2']=54,['3']=55,['4']=56,['5']=57,['6']=58,['7']=59,
        ['8']=60,['9']=61,['+']=62,['/']=63,
    };
    size_t out = 0;
    for (size_t i = 0; i < b64_len; i += 4) {
        int a = b64tab[(uint8_t)b64_start[i]];
        int b = b64tab[(uint8_t)b64_start[i + 1]];
        int c = (i + 2 < b64_len && b64_start[i + 2] != '=') ? b64tab[(uint8_t)b64_start[i + 2]] : -1;
        int d = (i + 3 < b64_len && b64_start[i + 3] != '=') ? b64tab[(uint8_t)b64_start[i + 3]] : -1;
        body[out++] = (char)((a << 2) | (b >> 4));
        if (c >= 0) body[out++] = (char)(((b & 0xf) << 4) | (c >> 2));
        if (d >= 0) body[out++] = (char)(((c & 0x3) << 6) | d);
    }
    body[out] = '\0';
    cloudsync_memory_free(resp);

    // Always NUL-terminate (per the contract memory rule); body was allocated
    // with one extra byte and zeroed. xfree/xdata stay NULL → the core frees
    // with cloudsync_memory_free (= sqlite3_free).
    (void)zero_terminated;
    return (NETWORK_RESULT){CLOUDSYNC_NETWORK_BUFFER, body, out, NULL, NULL};
}

bool network_send_buffer(network_data *data, const char *endpoint,
                         const char *authentication, const void *blob, int blob_size) {
    (void)data;
    (void)authentication;   // upload step passes NULL auth (pre-signed URL)

    if (!endpoint || !blob || blob_size <= 0) return false;

    char host[256];
    int port;
    // The endpoint is the `mem://<node-id-fingerprint>/<id>` URL the broker
    // returned from the upload step. The C layer does NOT parse the node id —
    // it routes through the local agent, which reads the mem:// authority and
    // dials the right peer over iroh.
    if (!resolve_agent_addr(host, sizeof(host), &port)) {
        return false;
    }

    // SYNC-5: bearer token for the C↔agent socket (same as receive_buffer).
    char token[256];
    if (!resolve_agent_token(token, sizeof(token))) {
        return false;
    }

    int fd = connect_tcp(host, port);
    if (fd < 0) return false;

    size_t frame_len = 0;
    char *frame = build_put_frame(endpoint, blob, blob_size, token, &frame_len);
    if (!frame) { close(fd); return false; }

    bool ok = write_frame(fd, frame, frame_len);
    cloudsync_memory_free(frame);
    if (!ok) { close(fd); return false; }

    // Read the PutResponse frame: {"ok":true,...}. Just need a frame back.
    size_t resp_len = 0;
    char *resp = read_frame(fd, &resp_len);
    close(fd);
    if (!resp) return false;

    bool ok_flag = strstr(resp, "\"ok\":true") != NULL;
    cloudsync_memory_free(resp);
    return ok_flag;
}