//
//  fractional_indexing.c
//  fractional_indexing
//
//  Created by Marco Bambini on 26/08/23.
//

// theory
// https://observablehq.com/@dgreensp/implementing-fractional-indexing
// https://www.figma.com/blog/realtime-editing-of-ordered-sequences/
// https://www.steveruiz.me/posts/reordering-fractional-indices
// https://vlcn.io/blog/fractional-indexing
// https://madebyevan.com/algos/crdt-fractional-indexing/

#include "fractional_indexing.h"
#include <string.h>
#include <stdlib.h>
#include <stdio.h>
#include <math.h>

// MARK: - Custom allocator -

static void *default_malloc (size_t size)              { return malloc(size); }
static void *default_calloc (size_t count, size_t size){ return calloc(count, size); }
static void  default_free   (void *ptr)                { free(ptr); }

static fractional_indexing_allocator fi_alloc = {
    default_malloc,
    default_calloc,
    default_free
};

void fractional_indexing_set_allocator (const fractional_indexing_allocator *alloc) {
    if (alloc) {
        fi_alloc.malloc = alloc->malloc ? alloc->malloc : default_malloc;
        fi_alloc.calloc = alloc->calloc ? alloc->calloc : default_calloc;
        fi_alloc.free   = alloc->free   ? alloc->free   : default_free;
    } else {
        fi_alloc.malloc = default_malloc;
        fi_alloc.calloc = default_calloc;
        fi_alloc.free   = default_free;
    }
}

#define FI_MALLOC(size)         fi_alloc.malloc(size)
#define FI_CALLOC(count, size)  fi_alloc.calloc(count, size)
#define FI_FREE(ptr)            fi_alloc.free(ptr)

static char *fi_strdup (const char *s) {
    size_t len = strlen(s) + 1;
    char *d = FI_MALLOC(len);
    if (d) memcpy(d, s, len);
    return d;
}

#ifndef max
#define max(a,b)             \
({                           \
    __typeof__ (a) _a = (a); \
    __typeof__ (b) _b = (b); \
    _a > _b ? _a : _b;       \
})
#endif

#define SMALLEST_INTEGER_BASE_10    "A00000000000000000000000000"
#define SMALLEST_INTEGER_BASE_62    "A00000000000000000000000000"
#define SMALLEST_INTEGER_BASE_95    "A                          "

#define INTEGER_ZERO_BASE_10        "a0"
#define INTEGER_ZERO_BASE_62        "a0"
#define INTEGER_ZERO_BASE_95        "a "

static const char *BASE_10_DIGITS = "0123456789";
static const char *BASE_62_DIGITS = "0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz";
static const char *BASE_95_DIGITS = " !\"#$%&'()*+,-./0123456789:;<=>?@ABCDEFGHIJKLMNOPQRSTUVWXYZ[\\]^_`abcdefghijklmnopqrstuvwxyz{|}~";

static size_t BASE_10_DIGITS_LEN = 10;
static size_t BASE_62_DIGITS_LEN = 62;
static size_t BASE_95_DIGITS_LEN = 95;

// Error strings (static, do not free)
static const char *ERR_INVALID_ORDER_KEY = "invalid order key";
static const char *ERR_A_GTE_B = "a >= b";
static const char *ERR_CANNOT_DECREMENT = "cannot decrement any more";
static const char *ERR_CANNOT_INCREMENT = "cannot increment any more";

// MARK: - Utilities -

inline static int index_of (const char *digits, size_t lenD, char c) {
    int index = 0;
    while (index < lenD) {
        if (digits[index] == c) return index;
        ++index;
    }
    return -1;
}

static char *concat_key (const char *prefix, int prefix_len, const char *suffix) {
    int suffix_len = (int)strlen(suffix);
    char *result = FI_MALLOC(prefix_len + suffix_len + 1);
    if (!result) return NULL;
    memcpy(result, prefix, prefix_len);
    memcpy(result + prefix_len, suffix, suffix_len + 1);
    return result;
}

static void build_constants (const char *digits, char *integer_zero, char *smallest_integer) {
    integer_zero[0] = 'a';
    integer_zero[1] = digits[0];
    integer_zero[2] = '\0';

    smallest_integer[0] = 'A';
    memset(smallest_integer + 1, digits[0], 26);
    smallest_integer[27] = '\0';
}

// MARK: - Midpoint -

static char *fractional_indexing_sanitycheck (const char *a, size_t *lenA, const char *b, size_t *lenB, const char zero, char **error) {
    // a cannot be NULL
    if (!a) {if (error) *error = "Error: a cannot be NULL"; return NULL;}

    // a < b lexicographically if b is non-null
    if (b != NULL && strcmp(a, b) >= 0) {if (error) *error = "Error: a >= b"; return NULL;}

    // compute lengths
    size_t _lenA = (a) ? strlen(a) : 0;
    size_t _lenB = (b) ? strlen(b) : 0;
    *lenA = _lenA;
    *lenB = _lenB;

    // no trailing zeros allowed
    char lastA = (_lenA) ? a[_lenA-1] : 0;
    char lastB = (b) ? b[_lenB-1] : 0;
    if (lastA == zero || lastB == zero) {if (error) *error = "Error: trailing zero"; return NULL;}

    size_t bsize = max(_lenA, _lenB) + 1;
    char *buffer = FI_CALLOC(1, bsize + 1);
    if (!buffer) {if (error) *error = "Not enough memory to allocate buffer"; return NULL;}

    return buffer;
}

static char *fractional_indexing_midpoint (const char *a, size_t lenA, const char *b, size_t lenB, const char *digits, size_t lenD, char *buffer, int *bindex) {
    int zero = digits[0];

    if (b) {
        // Remove the longest common prefix. Pad 'a' with 0s as we go.
        // Note that we don't need to pad 'b' because it can't end before 'a'
        // while traversing the common prefix.
        int n = 0;
        while (1) {
            int aChar = (n >= lenA) ? zero : a[n];
            int bChar = (n >= lenB) ? zero : b[n];
            if (aChar == bChar) ++n;
            else break;
        }
        if (n > 0) {
            memcpy(buffer + *bindex, b, n);
            const char *ptrA = (lenA > n) ? a+n : "";
            const char *ptrB = (lenB > n) ? b+n : "";
            lenA = strlen(ptrA);
            lenB = strlen(ptrB);
            *bindex = *bindex + n;
            return fractional_indexing_midpoint(ptrA, lenA, ptrB, lenB, digits, lenD, buffer, bindex);
        }
    }

    // Find the first digits (or lack of digit) in both strings
    int digitA = (a[0] != '\0') ? index_of(digits, lenD, a[0]) : 0;
    int digitB = (b && b[0] != '\0') ? index_of(digits, lenD, b[0]) : (int)lenD;

    if (digitB - digitA > 1) {
        // Midpoint found
        int midDigit = round(0.5 * (digitA + digitB));
        char midpointChar = digits[midDigit];
        buffer[*bindex] = midpointChar;
        *bindex = *bindex + 1;
        return buffer;
    } else {
        if (b && lenB > 1) {
            buffer[*bindex] = b[0];
            *bindex = *bindex + 1;
            return buffer;
        } else {
            // `b` is null or has length 1 (a single digit).
            // the first digit of `a` is the previous digit to `b`,
            // or 9 if `b` is null.
            // given, for example, midpoint('49', '5'), return
            // '4' + midpoint('9', null), which will become
            // '4' + '9' + midpoint('', null), which is '495'

            buffer[*bindex] = digits[digitA];
            *bindex = *bindex + 1;

            const char *ptrA = (lenA > 0) ? a+1 : a;
            return fractional_indexing_midpoint(ptrA, (lenA) ? lenA-1 : 0, NULL, 0, digits, lenD, buffer, bindex);
        }
    }

    return NULL;
}

static char *midpoint_alloc (const char *a, const char *b, const char *digits, size_t digitsLen) {
    size_t lenA = a ? strlen(a) : 0;
    size_t lenB = b ? strlen(b) : 0;
    if (!a) a = "";

    size_t bsize = max(lenA, lenB) + 1;
    char *buffer = FI_CALLOC(1, bsize + 1);
    if (!buffer) return NULL;

    int bindex = 0;
    return fractional_indexing_midpoint(a, lenA, b, lenB, digits, digitsLen, buffer, &bindex);
}

// MARK: - Public midpoint -

char *midpoint_base10 (const char *a, const char *b, char **error) {
    size_t lenA, lenB;
    char *buffer = fractional_indexing_sanitycheck(a, &lenA, b, &lenB, BASE_10_DIGITS[0], error);
    if (buffer == NULL) return NULL;

    int bindex = 0;
    return fractional_indexing_midpoint(a, lenA, b, lenB, BASE_10_DIGITS, BASE_10_DIGITS_LEN, buffer, &bindex);
}

char *midpoint_base62 (const char *a, const char *b, char **error) {
    size_t lenA, lenB;
    char *buffer = fractional_indexing_sanitycheck(a, &lenA, b, &lenB, BASE_62_DIGITS[0], error);
    if (buffer == NULL) return NULL;

    int bindex = 0;
    return fractional_indexing_midpoint(a, lenA, b, lenB, BASE_62_DIGITS, BASE_62_DIGITS_LEN, buffer, &bindex);
}

char *midpoint_base95 (const char *a, const char *b, char **error) {
    size_t lenA, lenB;
    char *buffer = fractional_indexing_sanitycheck(a, &lenA, b, &lenB, BASE_95_DIGITS[0], error);
    if (buffer == NULL) return NULL;

    int bindex = 0;
    return fractional_indexing_midpoint(a, lenA, b, lenB, BASE_95_DIGITS, BASE_95_DIGITS_LEN, buffer, &bindex);
}

// MARK: - Integer part helpers -

static int get_integer_length (const char *head) {
    char firstChar = head[0];
    if (firstChar >= 'a' && firstChar <= 'z') {
        return firstChar - 'a' + 2;
    } else if (firstChar >= 'A' && firstChar <= 'Z') {
        return 'Z' - firstChar + 2;
    }

    return -1;
}

static bool validate_order_key_internal (const char *key, const char *smallest_integer, char zero) {
    if (!key) return false;
    int keyLen = (int)strlen(key);
    if (keyLen == 0) return false;

    if (strcmp(key, smallest_integer) == 0) return false;

    int intLen = get_integer_length(key);
    if (intLen == -1 || intLen > keyLen) return false;

    // fractional part must not end with zero digit (no trailing zeros)
    if (keyLen > intLen && key[keyLen - 1] == zero) return false;

    return true;
}

static bool validate_integer (const char *value) {
    int len = (value) ? (int)strlen(value) : 0;
    if (len == 0) return false;
    return (len == get_integer_length(value));
}

// MARK: - Increment / Decrement -

static char *increment_integer_internal (const char *x, const char *digits, int digitsLen) {
    if (!validate_integer(x)) return NULL;

    int xlen = (int)strlen(x);
    if (xlen < 2) return NULL;

    char head = x[0];
    char digs[256] = {0};
    memcpy(digs, x+1, xlen -1);
    int digsLen = (int)strlen(digs);

    bool carry = true;
    for (int i = digsLen -1; i >= 0; --i) {
        if (!carry) break;

        int d = index_of(digits, digitsLen, digs[i]) + 1;
        if (d == digitsLen) {
            digs[i] = digits[0];
        } else {
            digs[i] = digits[d];
            carry = false;
        }
    }

    if (carry) {
        if (head == 'Z') {
            char *result = (char *)FI_MALLOC(3);
            if (!result) return NULL;
            result[0] = 'a';
            result[1] = digits[0];
            result[2] = '\0';
            return result;
        } else if (head == 'z') {
            return NULL;
        }

        int h = head + 1;
        if (h > 'a') {
            digs[digsLen] = digits[0];
        } else {
            digs[digsLen-1] = 0;
        }

        char *result = (char *)FI_MALLOC(xlen + 2);
        if (!result) return NULL;

        result[0] = h;
        strcpy(result + 1, digs);
        return result;
    }

    // carry is false
    char* result = (char *)FI_MALLOC(xlen + 1);
    if (!result) return NULL;

    result[0] = head;
    strcpy(result + 1, digs);
    return result;
}

static char *decrement_integer_internal (const char *x, const char *digits, int digitsLen) {
    if (!validate_integer(x)) return NULL;

    int xlen = (int)strlen(x);
    if (xlen < 2) return NULL;

    char head = x[0];
    char digs[256] = {0};
    memcpy(digs, x+1, xlen -1);
    int digsLen = (int)strlen(digs);

    bool borrow = true;
    for (int i = digsLen -1; i >= 0; --i) {
        if (!borrow) break;

        int d = index_of(digits, digitsLen, digs[i]) - 1;
        if (d == -1) {
            digs[i] = digits[digitsLen-1];
        } else {
            digs[i] = digits[d];
            borrow = false;
        }
    }

    if (borrow) {
        if (head == 'a') {
            char *result = (char *)FI_MALLOC(3);
            if (!result) return NULL;
            result[0] = 'Z';
            result[1] = digits[digitsLen-1];
            result[2] = '\0';
            return result;
        } else if (head == 'A') {
            return NULL;
        }

        int h = head - 1;
        if (h < 'Z') {
            digs[digsLen] = digits[digitsLen-1];
        } else {
            digs[digsLen-1] = 0;
        }

        char *result = (char *)FI_MALLOC(xlen + 2);
        if (!result) return NULL;

        result[0] = h;
        strcpy(result + 1, digs);
        return result;
    }

    // borrow is false
    char* result = (char *)FI_MALLOC(xlen + 1);
    if (!result) return NULL;

    result[0] = head;
    strcpy(result + 1, digs);
    return result;
}

// MARK: - Generate key between -

static char *generate_key_between_internal (const char *a, const char *b, const char *integer_zero, const char *smallest_integer, const char *digits, int digitsLen, char **err) {
    if (a) {
        if (!validate_order_key_internal(a, smallest_integer, digits[0])) {
            if (err) *err = (char *)ERR_INVALID_ORDER_KEY;
            return NULL;
        }
    }

    if (b) {
        if (!validate_order_key_internal(b, smallest_integer, digits[0])) {
            if (err) *err = (char *)ERR_INVALID_ORDER_KEY;
            return NULL;
        }
    }

    if (a && b && strcmp(a, b) >= 0) {
        if (err) *err = (char *)ERR_A_GTE_B;
        return NULL;
    }

    if (a == NULL && b == NULL) {
        return fi_strdup(integer_zero);
    }

    if (a == NULL) {
        int ibLen = get_integer_length(b);
        const char *fb = b + ibLen;

        // Check if integer part equals smallest_integer
        if (ibLen == (int)strlen(smallest_integer) && strncmp(b, smallest_integer, ibLen) == 0) {
            char *mid = midpoint_alloc("", fb, digits, digitsLen);
            if (!mid) return NULL;
            char *result = concat_key(b, ibLen, mid);
            FI_FREE(mid);
            return result;
        }

        // If b has a fractional part, integer part alone is between NULL and b
        if (*fb != '\0') {
            char *result = FI_MALLOC(ibLen + 1);
            if (!result) return NULL;
            memcpy(result, b, ibLen);
            result[ibLen] = '\0';
            return result;
        }

        // Decrement the integer part
        char ib_buf[64] = {0};
        memcpy(ib_buf, b, ibLen);
        char *dec = decrement_integer_internal(ib_buf, digits, digitsLen);
        if (!dec && err) *err = (char *)ERR_CANNOT_DECREMENT;
        return dec;
    }

    if (b == NULL) {
        int iaLen = get_integer_length(a);
        const char *fa = a + iaLen;

        char ia_buf[64] = {0};
        memcpy(ia_buf, a, iaLen);

        char *i = increment_integer_internal(ia_buf, digits, digitsLen);
        if (i != NULL) return i;

        // Cannot increment, use ia + midpoint(fa, NULL)
        char *mid = midpoint_alloc(fa, NULL, digits, digitsLen);
        if (!mid) return NULL;
        char *result = concat_key(a, iaLen, mid);
        FI_FREE(mid);
        return result;
    }

    // Both a and b are non-NULL
    int iaLen = get_integer_length(a);
    const char *fa = a + iaLen;
    int ibLen = get_integer_length(b);
    const char *fb = b + ibLen;

    if (iaLen == ibLen && strncmp(a, b, iaLen) == 0) {
        // Same integer part: return ia + midpoint(fa, fb)
        char *mid = midpoint_alloc(fa, fb, digits, digitsLen);
        if (!mid) return NULL;
        char *result = concat_key(a, iaLen, mid);
        FI_FREE(mid);
        return result;
    }

    char ia_buf[64] = {0};
    memcpy(ia_buf, a, iaLen);

    char *i = increment_integer_internal(ia_buf, digits, digitsLen);
    if (i == NULL) {
        if (err) *err = (char *)ERR_CANNOT_INCREMENT;
        return NULL;
    }
    if (strcmp(i, b) < 0) {
        return i;
    }
    FI_FREE(i);

    // return ia + midpoint(fa, NULL)
    char *mid = midpoint_alloc(fa, NULL, digits, digitsLen);
    if (!mid) return NULL;
    char *result = concat_key(a, iaLen, mid);
    FI_FREE(mid);
    return result;
}

// MARK: - Generate N keys between -

static char **generate_n_keys_between_internal (const char *a, const char *b, int n, const char *integer_zero, const char *smallest_integer, const char *digits, int digitsLen, char **err) {
    if (n == 0) return NULL;

    char **result = FI_CALLOC(n, sizeof(char *));
    if (!result) return NULL;

    if (n == 1) {
        result[0] = generate_key_between_internal(a, b, integer_zero, smallest_integer, digits, digitsLen, err);
        if (!result[0]) { FI_FREE(result); return NULL; }
        return result;
    }

    if (b == NULL) {
        char *c = generate_key_between_internal(a, b, integer_zero, smallest_integer, digits, digitsLen, err);
        if (!c) { FI_FREE(result); return NULL; }
        result[0] = c;
        for (int i = 1; i < n; i++) {
            c = generate_key_between_internal(result[i-1], b, integer_zero, smallest_integer, digits, digitsLen, err);
            if (!c) { for (int j = 0; j < i; j++) FI_FREE(result[j]); FI_FREE(result); return NULL; }
            result[i] = c;
        }
        return result;
    }

    if (a == NULL) {
        char *c = generate_key_between_internal(a, b, integer_zero, smallest_integer, digits, digitsLen, err);
        if (!c) { FI_FREE(result); return NULL; }
        result[0] = c;
        for (int i = 1; i < n; i++) {
            c = generate_key_between_internal(a, result[i-1], integer_zero, smallest_integer, digits, digitsLen, err);
            if (!c) { for (int j = 0; j < i; j++) FI_FREE(result[j]); FI_FREE(result); return NULL; }
            result[i] = c;
        }
        // reverse to get ascending order
        for (int i = 0; i < n / 2; i++) {
            char *tmp = result[i];
            result[i] = result[n - 1 - i];
            result[n - 1 - i] = tmp;
        }
        return result;
    }

    // Both a and b non-null: divide and conquer
    int mid = n / 2;
    char *c = generate_key_between_internal(a, b, integer_zero, smallest_integer, digits, digitsLen, err);
    if (!c) { FI_FREE(result); return NULL; }

    char **left = NULL;
    if (mid > 0) {
        left = generate_n_keys_between_internal(a, c, mid, integer_zero, smallest_integer, digits, digitsLen, err);
        if (!left) { FI_FREE(c); FI_FREE(result); return NULL; }
    }

    int rightN = n - mid - 1;
    char **right = NULL;
    if (rightN > 0) {
        right = generate_n_keys_between_internal(c, b, rightN, integer_zero, smallest_integer, digits, digitsLen, err);
        if (!right) {
            if (left) { for (int j = 0; j < mid; j++) FI_FREE(left[j]); FI_FREE(left); }
            FI_FREE(c); FI_FREE(result); return NULL;
        }
    }

    int idx = 0;
    for (int i = 0; i < mid; i++) result[idx++] = left[i];
    result[idx++] = c;
    for (int i = 0; i < rightN; i++) result[idx++] = right[i];

    if (left) FI_FREE(left);
    if (right) FI_FREE(right);

    return result;
}

// MARK: - Public API: Increment / Decrement -

char *increment_integer_base10 (const char *x) {
    return increment_integer_internal(x, BASE_10_DIGITS, (int)BASE_10_DIGITS_LEN);
}

char *increment_integer_base62 (const char *x) {
    return increment_integer_internal(x, BASE_62_DIGITS, (int)BASE_62_DIGITS_LEN);
}

char *increment_integer_base95 (const char *x) {
    return increment_integer_internal(x, BASE_95_DIGITS, (int)BASE_95_DIGITS_LEN);
}

char *decrement_integer_base10 (const char *x) {
    return decrement_integer_internal(x, BASE_10_DIGITS, (int)BASE_10_DIGITS_LEN);
}

char *decrement_integer_base62 (const char *x) {
    return decrement_integer_internal(x, BASE_62_DIGITS, (int)BASE_62_DIGITS_LEN);
}

char *decrement_integer_base95 (const char *x) {
    return decrement_integer_internal(x, BASE_95_DIGITS, (int)BASE_95_DIGITS_LEN);
}

// MARK: - Public API: Validation -

bool validate_order_key_base10 (const char *key) {
    return validate_order_key_internal(key, SMALLEST_INTEGER_BASE_10, BASE_10_DIGITS[0]);
}

bool validate_order_key_base62 (const char *key) {
    return validate_order_key_internal(key, SMALLEST_INTEGER_BASE_62, BASE_62_DIGITS[0]);
}

bool validate_order_key_base95 (const char *key) {
    return validate_order_key_internal(key, SMALLEST_INTEGER_BASE_95, BASE_95_DIGITS[0]);
}

bool validate_order_key_custom (const char *key, const char *digits) {
    char smallest_integer[28];
    smallest_integer[0] = 'A';
    memset(smallest_integer + 1, digits[0], 26);
    smallest_integer[27] = '\0';
    return validate_order_key_internal(key, smallest_integer, digits[0]);
}

// MARK: - Public API: Generate key between -

char *generate_key_between_base10 (const char *a, const char *b) {
    return generate_key_between_internal(a, b, INTEGER_ZERO_BASE_10, SMALLEST_INTEGER_BASE_10, BASE_10_DIGITS, (int)BASE_10_DIGITS_LEN, NULL);
}

char *generate_key_between_base62 (const char *a, const char *b) {
    return generate_key_between_internal(a, b, INTEGER_ZERO_BASE_62, SMALLEST_INTEGER_BASE_62, BASE_62_DIGITS, (int)BASE_62_DIGITS_LEN, NULL);
}

char *generate_key_between_base95 (const char *a, const char *b) {
    return generate_key_between_internal(a, b, INTEGER_ZERO_BASE_95, SMALLEST_INTEGER_BASE_95, BASE_95_DIGITS, (int)BASE_95_DIGITS_LEN, NULL);
}

char *generate_key_between_custom (const char *a, const char *b, const char *digits, char **error) {
    char integer_zero[3];
    char smallest_integer[28];
    build_constants(digits, integer_zero, smallest_integer);
    return generate_key_between_internal(a, b, integer_zero, smallest_integer, digits, (int)strlen(digits), error);
}

// MARK: - Public API: Generate N keys between -

char **generate_n_keys_between_base10 (const char *a, const char *b, int n) {
    return generate_n_keys_between_internal(a, b, n, INTEGER_ZERO_BASE_10, SMALLEST_INTEGER_BASE_10, BASE_10_DIGITS, (int)BASE_10_DIGITS_LEN, NULL);
}

char **generate_n_keys_between_base62 (const char *a, const char *b, int n) {
    return generate_n_keys_between_internal(a, b, n, INTEGER_ZERO_BASE_62, SMALLEST_INTEGER_BASE_62, BASE_62_DIGITS, (int)BASE_62_DIGITS_LEN, NULL);
}

char **generate_n_keys_between_base95 (const char *a, const char *b, int n) {
    return generate_n_keys_between_internal(a, b, n, INTEGER_ZERO_BASE_95, SMALLEST_INTEGER_BASE_95, BASE_95_DIGITS, (int)BASE_95_DIGITS_LEN, NULL);
}

char **generate_n_keys_between_custom (const char *a, const char *b, int n, const char *digits, char **error) {
    char integer_zero[3];
    char smallest_integer[28];
    build_constants(digits, integer_zero, smallest_integer);
    return generate_n_keys_between_internal(a, b, n, integer_zero, smallest_integer, digits, (int)strlen(digits), error);
}

// MARK: -

void free_keys (char **keys, int n) {
    if (!keys) return;
    for (int i = 0; i < n; i++) FI_FREE(keys[i]);
    FI_FREE(keys);
}

// MARK: - Default API (base62)

char *midpoint (const char *a, const char *b, char **error) {
    return midpoint_base62(a, b, error);
}

char *increment_integer (const char *x) {
    return increment_integer_base62(x);
}

char *decrement_integer (const char *x) {
    return decrement_integer_base62(x);
}

bool validate_order_key (const char *key) {
    return validate_order_key_base62(key);
}

char *generate_key_between (const char *a, const char *b) {
    return generate_key_between_base62(a, b);
}

char **generate_n_keys_between (const char *a, const char *b, int n) {
    return generate_n_keys_between_base62(a, b, n);
}
