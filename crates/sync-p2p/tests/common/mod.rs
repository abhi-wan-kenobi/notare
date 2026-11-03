//! In-memory [`ChangeSource`] that behaves like cr-sqlite where it matters to
//! the transport: applied changes get new local `db_version`s, `apply` is
//! idempotent, and `changes_since` honours cursor, site exclusion and limit.
#![allow(dead_code)]

use std::collections::{HashMap, HashSet};
use std::sync::Mutex;

use sync_p2p::{ChangeSource, ChangeSourceError, WireChange, WireCursor, WirePage, WireValue};

#[derive(Default)]
struct State {
    rows: Vec<Row>,
    seen: HashSet<(Vec<u8>, i64, i64)>,
    version: i64,
    cursors: HashMap<String, WireCursor>,
    cursor_history: Vec<(String, WireCursor)>,
    pull_log: Vec<(WireCursor, Option<Vec<u8>>, usize)>,
    apply_pages: usize,
    apply_calls: usize,
    changes_since_calls: usize,
    fail_apply_on_page: Option<usize>,
    fail_changes_since_on_call: Option<usize>,
}

struct Row {
    change: WireChange,
}

pub struct FakeSource {
    site: Vec<u8>,
    state: Mutex<State>,
}

pub fn change_cursor(c: &WireChange) -> WireCursor {
    WireCursor {
        db_version: c.db_version,
        seq: c.seq,
    }
}

impl FakeSource {
    pub fn new(site_byte: u8) -> Self {
        Self {
            site: vec![site_byte; 16],
            state: Mutex::new(State::default()),
        }
    }

    pub fn site(&self) -> Vec<u8> {
        self.site.clone()
    }

    /// One local write: new db_version, origin = this site.
    pub fn write(&self, table: &str, pk: &str, val: &str) {
        let mut s = self.state.lock().unwrap();
        s.version += 1;
        let change = WireChange {
            table: table.into(),
            pk: WireValue::Text(pk.into()),
            cid: "v".into(),
            val: WireValue::Text(val.into()),
            col_version: 1,
            db_version: s.version,
            site_id: self.site.clone(),
            cl: 1,
            seq: 0,
        };
        s.seen
            .insert((self.site.clone(), change.db_version, change.seq));
        s.rows.push(Row { change });
    }

    pub fn write_many(&self, table: &str, prefix: &str, n: usize) {
        for i in 0..n {
            self.write(table, &format!("{prefix}{i}"), "x");
        }
    }

    /// Primary keys of every row held, sorted.
    pub fn pks(&self) -> Vec<String> {
        let s = self.state.lock().unwrap();
        let mut v: Vec<String> = s
            .rows
            .iter()
            .map(|r| match &r.change.pk {
                WireValue::Text(t) => t.clone(),
                other => panic!("unexpected pk {other:?}"),
            })
            .collect();
        v.sort();
        v
    }

    pub fn row_count(&self) -> usize {
        self.state.lock().unwrap().rows.len()
    }

    pub fn cursor_for(&self, peer: &str) -> WireCursor {
        self.state
            .lock()
            .unwrap()
            .cursors
            .get(peer)
            .copied()
            .unwrap_or_default()
    }

    pub fn cursor_history(&self) -> Vec<WireCursor> {
        self.state
            .lock()
            .unwrap()
            .cursor_history
            .iter()
            .map(|(_, c)| *c)
            .collect()
    }

    /// `(after, exclude_site, limit)` of every `changes_since` call received.
    pub fn pull_log(&self) -> Vec<(WireCursor, Option<Vec<u8>>, usize)> {
        self.state.lock().unwrap().pull_log.clone()
    }

    pub fn apply_calls(&self) -> usize {
        self.state.lock().unwrap().apply_calls
    }

    pub fn changes_since_calls(&self) -> usize {
        self.state.lock().unwrap().changes_since_calls
    }

    /// Make the `n`th (0-based) `apply` call fail; `None` clears it.
    pub fn fail_apply_on_page(&self, n: Option<usize>) {
        let mut s = self.state.lock().unwrap();
        s.fail_apply_on_page = n;
        s.apply_pages = 0;
    }

    /// Make the `n`th (0-based) `changes_since` call fail.
    pub fn fail_changes_since_on_call(&self, n: Option<usize>) {
        let mut s = self.state.lock().unwrap();
        s.fail_changes_since_on_call = n;
        s.changes_since_calls = 0;
    }
}

impl ChangeSource for FakeSource {
    async fn site_id(&self) -> Result<Vec<u8>, ChangeSourceError> {
        Ok(self.site.clone())
    }

    async fn changes_since(
        &self,
        after: WireCursor,
        exclude_site: Option<Vec<u8>>,
        limit: usize,
    ) -> Result<WirePage, ChangeSourceError> {
        let mut s = self.state.lock().unwrap();
        let call = s.changes_since_calls;
        s.changes_since_calls += 1;
        s.pull_log.push((after, exclude_site.clone(), limit));
        if s.fail_changes_since_on_call == Some(call) {
            return Err(ChangeSourceError(
                "secret table internals: SELECT * FROM hidden".into(),
            ));
        }
        let mut eligible: Vec<&WireChange> = s
            .rows
            .iter()
            .map(|r| &r.change)
            .filter(|c| change_cursor(c) > after)
            .filter(|c| exclude_site.as_ref() != Some(&c.site_id))
            .collect();
        eligible.sort_by_key(|c| change_cursor(c));
        let more = eligible.len() > limit;
        let changes: Vec<WireChange> = eligible.into_iter().take(limit).cloned().collect();
        let next = changes.last().map(change_cursor).unwrap_or(after);
        Ok(WirePage {
            changes,
            next,
            more,
        })
    }

    async fn apply(&self, changes: Vec<WireChange>) -> Result<(), ChangeSourceError> {
        let mut s = self.state.lock().unwrap();
        s.apply_calls += 1;
        let page = s.apply_pages;
        s.apply_pages += 1;
        if s.fail_apply_on_page == Some(page) {
            return Err(ChangeSourceError("disk full".into()));
        }
        for mut c in changes {
            let origin = (c.site_id.clone(), c.db_version, c.seq);
            if !s.seen.insert(origin) {
                continue;
            }
            s.version += 1;
            c.db_version = s.version;
            s.rows.push(Row { change: c });
        }
        Ok(())
    }

    async fn peer_cursor(&self, peer: &str) -> Result<WireCursor, ChangeSourceError> {
        Ok(self.cursor_for(peer))
    }

    async fn set_peer_cursor(
        &self,
        peer: &str,
        cursor: WireCursor,
    ) -> Result<(), ChangeSourceError> {
        let mut s = self.state.lock().unwrap();
        s.cursors.insert(peer.to_string(), cursor);
        s.cursor_history.push((peer.to_string(), cursor));
        Ok(())
    }
}
