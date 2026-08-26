// S0b placeholder custom network layer.
//
// Building the extension with -DCLOUDSYNC_OMIT_CURL removes the default
// libcurl implementation of the two functions below, but the CloudSync core
// still *calls* them. A shared object with undefined symbols fails to dlopen
// (SQLite uses RTLD_NOW), so a from-source build must ship at least these
// stubs. They are the exact replace-point for S1's real P2P transport —
// swap this file for the real implementation, nothing else in the core changes.

#include <stdbool.h>
#include <stddef.h>

#include "network_private.h"

bool network_send_buffer(
    network_data *data,
    const char *endpoint,
    const char *authentication,
    const void *blob,
    int blob_size
) {
    (void)data;
    (void)endpoint;
    (void)authentication;
    (void)blob;
    (void)blob_size;
    return false;
}

NETWORK_RESULT network_receive_buffer(
    network_data *data,
    const char *endpoint,
    const char *authentication,
    bool zero_terminated,
    bool is_post_request,
    char *json_payload,
    const char *custom_header
) {
    (void)data;
    (void)endpoint;
    (void)authentication;
    (void)zero_terminated;
    (void)is_post_request;
    (void)json_payload;
    (void)custom_header;
    return (NETWORK_RESULT){CLOUDSYNC_NETWORK_ERROR, NULL, 0, NULL, NULL};
}
