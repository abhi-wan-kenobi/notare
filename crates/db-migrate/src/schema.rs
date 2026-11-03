#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MigrationScope {
    Plain,
    /// DDL on a synced table. Runs inside a cr-sqlite alter window when the
    /// table is already a CRR; see `hypr_db_core::Db::begin_crr_alter_on`.
    CrrAlter {
        table_name: &'static str,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MigrationStep {
    pub id: &'static str,
    pub scope: MigrationScope,
    pub sql: &'static str,
}

#[derive(Clone, Copy)]
pub struct DbSchema {
    pub steps: &'static [MigrationStep],
    /// Whether `table_name` may be the target of a `CrrAlter` step.
    pub validate_crr_table: fn(&str) -> bool,
}
