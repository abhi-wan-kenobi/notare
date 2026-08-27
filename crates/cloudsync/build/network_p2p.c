// S1 P2P network layer for the vendored sqlite-sync (CloudSync) extension.
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
// Transport: a framed length-prefixed TCP protocol on localhost to a Rust
// broker (crates/sync-p2p) that serves the CloudSync control protocol directly,
// collapsing the HTTP-S3 3-step upload/apply flow onto an in-memory object
// store. Pure POSIX sockets — no libcurl, no external deps. The broker is the
// "SQLite Cloud server" for the spike.
//
// SPIKE scope: localhost, unencrypted, no auth, blocking. Production transport
// (iroh/QUIC, NAT traversal, relay, encryption, pairing) is out of scope.

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
// endpoint parsing
// ------------------------------------------------------------------

// Parse `p2p://host:port/...` (or `http://...`) out of an endpoint URL into a
// host string and port. Returns false on any parse failure (defensive — the
// endpoint is attacker-influenced user input).
static bool parse_host_port(const char *endpoint, char *host_out, size_t host_cap, int *port_out) {
    if (!endpoint) return false;
    const char *s = endpoint;
    if (strncmp(s, "p2p://", 6) == 0) s += 6;
    else if (strncmp(s, "http://", 7) == 0) s += 7;
    else if (strncmp(s, "mem://", 6) == 0) s += 6;
    else return false;

    // Find the end of the host:port authority (next '/').
    const char *slash = strchr(s, '/');
    size_t auth_len = slash ? (size_t)(slash - s) : strlen(s);
    if (auth_len == 0 || auth_len >= host_cap) return false;

    // Split host:port at the last ':' (handles bracketed IPv6 too, but we only
    // need localhost/127.0.0.1 for the spike).
    const char *colon = NULL;
    for (size_t i = 0; i < auth_len; i++) {
        if (s[i] == ':') colon = &s[i];
    }
    if (!colon) return false; // no port

    size_t host_len = (size_t)(colon - s);
    if (host_len == 0 || host_len >= host_cap) return false;
    memcpy(host_out, s, host_len);
    host_out[host_len] = '\0';

    char port_buf[16];
    size_t port_len = auth_len - host_len - 1;
    if (port_len == 0 || port_len >= sizeof(port_buf)) return false;
    memcpy(port_buf, colon + 1, port_len);
    port_buf[port_len] = '\0';

    char *end = NULL;
    long port = strtol(port_buf, &end, 10);
    if (end == port_buf || *end != '\0' || port < 1 || port > 65535) return false;
    *port_out = (int)port;
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
static char *build_request_frame(const char *endpoint, bool is_post,
                                 const char *json_payload, size_t *out_len) {
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

    const char *body_json = body_b64 ? body_b64 : "null";
    const char *body_quoted = body_b64 ? "\"" : "";
    const char *body_close = body_b64 ? "\"" : "";
    size_t cap = strlen(ep_esc) + strlen(body_json) + 64;
    char *frame = (char *)cloudsync_memory_zeroalloc(cap);
    if (!frame) { cloudsync_memory_free(ep_esc); cloudsync_memory_free(body_b64); return NULL; }

    int n = snprintf(frame, cap,
        "{\"endpoint\":\"%s\",\"is_post\":%s,\"body\":%s%s%s}",
        ep_esc, is_post ? "true" : "false", body_quoted, body_json, body_close);
    cloudsync_memory_free(ep_esc);
    cloudsync_memory_free(body_b64);
    if (n < 0 || (size_t)n >= cap) { cloudsync_memory_free(frame); return NULL; }
    if (out_len) *out_len = (size_t)n;
    return frame;
}

// Build a PutRequest frame for network_send_buffer:
//   {"url":"<escaped endpoint>","blob":"<base64 blob>"}
static char *build_put_frame(const char *endpoint, const void *blob, int blob_size, size_t *out_len) {
    char *ep_esc = json_escape(endpoint);
    if (!ep_esc) return NULL;

    char *b64buf = base64_encode(blob, (size_t)blob_size);
    if (!b64buf) { cloudsync_memory_free(ep_esc); return NULL; }

    size_t cap = strlen(ep_esc) + strlen(b64buf) + 32;
    char *frame = (char *)cloudsync_memory_zeroalloc(cap);
    if (!frame) { cloudsync_memory_free(ep_esc); cloudsync_memory_free(b64buf); return NULL; }

    int n = snprintf(frame, cap, "{\"url\":\"%s\",\"blob\":\"%s\"}", ep_esc, b64buf);
    cloudsync_memory_free(ep_esc);
    cloudsync_memory_free(b64buf);
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
    if (!parse_host_port(endpoint, host, sizeof(host), &port)) {
        char *msg = cloudsync_string_dup("network_receive_buffer: bad endpoint");
        return (NETWORK_RESULT){CLOUDSYNC_NETWORK_ERROR, msg, 1, NULL, NULL};
    }

    int fd = connect_tcp(host, port);
    if (fd < 0) {
        char *msg = cloudsync_string_dup("network_receive_buffer: connect failed");
        return (NETWORK_RESULT){CLOUDSYNC_NETWORK_ERROR, msg, 1, NULL, NULL};
    }

    size_t frame_len = 0;
    char *frame = build_request_frame(endpoint, is_post_request, json_payload, &frame_len);
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

    // Find "body":"..."  or  "body":null
    char *key = strstr(resp, "\"body\"");
    if (!key) {
        cloudsync_memory_free(resp);
        char *msg = cloudsync_string_dup("network_receive_buffer: no body field");
        return (NETWORK_RESULT){CLOUDSYNC_NETWORK_ERROR, msg, 1, NULL, NULL};
    }
    char *val = key + 6;
    while (*val == ' ' || *val == ':' || *val == '\t') val++;
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
    // The endpoint is the `mem://127.0.0.1:PORT/<id>` URL the broker returned
    // from the upload step. parse_host_port handles the mem:// scheme.
    if (!parse_host_port(endpoint, host, sizeof(host), &port)) {
        return false;
    }

    int fd = connect_tcp(host, port);
    if (fd < 0) return false;

    size_t frame_len = 0;
    char *frame = build_put_frame(endpoint, blob, blob_size, &frame_len);
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