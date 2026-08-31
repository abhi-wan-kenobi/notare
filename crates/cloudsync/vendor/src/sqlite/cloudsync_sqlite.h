//
//  cloudsync_sqlite.h
//  cloudsync
//
//  Created by Marco Bambini on 05/12/25.
//

#ifndef __CLOUDSYNC_SQLITE__
#define __CLOUDSYNC_SQLITE__

#ifndef SQLITE_CORE
#include "sqlite3ext.h"
#else
#include "sqlite3.h"
#endif

// SYNC-9: MSVC requires a declaration's dllexport/dllimport linkage to match
// its definition's exactly (C2375 otherwise); cloudsync_sqlite.c defines
// sqlite3_cloudsync_init with APIEXPORT (__declspec(dllexport) on Windows),
// so the declaration needs the identical macro. GCC/clang never enforced
// this, which is why it was never noticed before a real MSVC build reached
// this file. See docs/internal/sync-p2p.md §21.15.
#ifdef _WIN32
#define APIEXPORT   __declspec(dllexport)
#else
#define APIEXPORT
#endif

APIEXPORT int sqlite3_cloudsync_init (sqlite3 *db, char **pzErrMsg, const sqlite3_api_routines *pApi);

#endif
