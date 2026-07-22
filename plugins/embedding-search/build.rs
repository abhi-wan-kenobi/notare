const COMMANDS: &[&str] = &[
    "embed_and_index_chunks",
    "delete_session_chunks",
    "semantic_search",
    "embedding_index_status",
];

fn main() {
    tauri_plugin::Builder::new(COMMANDS).build();
}
