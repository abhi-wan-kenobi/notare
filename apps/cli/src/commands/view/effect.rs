pub(crate) enum Effect {
    SaveMemo { session_id: String, memo: String },
    Exit,
}
