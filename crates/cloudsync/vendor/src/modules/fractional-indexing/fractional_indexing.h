//
//  fractional_indexing.h
//  fractional_indexing
//
//  Created by Marco Bambini on 26/08/23
//  (c) 2023 SQLite Cloud, Inc.
//

#ifndef __FRACTIONAL_INDEXING__
#define __FRACTIONAL_INDEXING__

#include <stdbool.h>
#include <stddef.h>

// Custom allocator
// Set before any library calls. If not set, stdlib malloc/calloc/free are used.
typedef struct {
    void *(*malloc)(size_t size);
    void *(*calloc)(size_t count, size_t size);
    void  (*free)(void *ptr);
} fractional_indexing_allocator;

void fractional_indexing_set_allocator (const fractional_indexing_allocator *alloc);

// Midpoint (low-level)
char *midpoint_base10 (const char *a, const char *b, char **error);
char *midpoint_base62 (const char *a, const char *b, char **error);
char *midpoint_base95 (const char *a, const char *b, char **error);

// Increment / Decrement integer part
char *increment_integer_base10 (const char *x);
char *increment_integer_base62 (const char *x);
char *increment_integer_base95 (const char *x);

char *decrement_integer_base10 (const char *x);
char *decrement_integer_base62 (const char *x);
char *decrement_integer_base95 (const char *x);

// Validation
bool validate_order_key_base10 (const char *key);
bool validate_order_key_base62 (const char *key);
bool validate_order_key_base95 (const char *key);

// Generate key between two keys (returns NULL on error)
char *generate_key_between_base10 (const char *a, const char *b);
char *generate_key_between_base62 (const char *a, const char *b);
char *generate_key_between_base95 (const char *a, const char *b);

// Generate N keys between two keys (returns NULL on error or n==0)
char **generate_n_keys_between_base10 (const char *a, const char *b, int n);
char **generate_n_keys_between_base62 (const char *a, const char *b, int n);
char **generate_n_keys_between_base95 (const char *a, const char *b, int n);

// Custom base API (digits must be in ascending character code order)
// error is optional: if non-NULL, set to a static string on failure
char *generate_key_between_custom (const char *a, const char *b, const char *digits, char **error);
char **generate_n_keys_between_custom (const char *a, const char *b, int n, const char *digits, char **error);
bool validate_order_key_custom (const char *key, const char *digits);

void free_keys (char **keys, int n);

// Default API (uses base62 — best choice for SQLite and general use)
char *midpoint (const char *a, const char *b, char **error);
char *increment_integer (const char *x);
char *decrement_integer (const char *x);
bool validate_order_key (const char *key);
char *generate_key_between (const char *a, const char *b);
char **generate_n_keys_between (const char *a, const char *b, int n);

#endif
