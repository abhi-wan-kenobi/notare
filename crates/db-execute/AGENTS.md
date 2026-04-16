# `db-execute`

## Role

- Reusable one-shot SQL execution over `db-core::Db`.
- Owns query execution, proxy query execution, JSON row serialization, and query-method parsing.
- Must stay transport-agnostic and non-reactive.

## Owns

- `DbExecutor`
- `ProxyQueryMethod`
- `ProxyQueryResult`
- SQL param binding from JSON values
- Named-row and positional-row serialization

## Does Not Own

- Subscription state or invalidation
- Dependency analysis or schema cataloging
- Tauri/mobile transport adapters

## Dependency Direction

- May depend on `db-core`
- May be consumed by `db-reactive`, `plugins/db`, and `mobile-bridge`
