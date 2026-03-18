pub(crate) fn command_match_score(query: &str, command: &str) -> Option<i32> {
    let query = query.trim().to_ascii_lowercase();
    if query.is_empty() {
        return Some(1);
    }

    let command = command.trim_start_matches('/').to_ascii_lowercase();

    let direct_score = single_command_match_score(&query, &command);
    let alias_score = command_aliases(&command)
        .iter()
        .filter_map(|alias| single_command_match_score(&query, alias).map(|score| score - 25))
        .max();

    match (direct_score, alias_score) {
        (Some(direct), Some(alias)) => Some(direct.max(alias)),
        (Some(direct), None) => Some(direct),
        (None, Some(alias)) => Some(alias),
        (None, None) => None,
    }
}

fn single_command_match_score(query: &str, command: &str) -> Option<i32> {
    if query.is_empty() {
        return Some(1);
    }

    if command.starts_with(query) {
        let penalty = (command.len() as i32 - query.len() as i32).max(0);
        return Some(500 - penalty);
    }

    if let Some(pos) = command.find(query) {
        return Some(350 - pos as i32);
    }

    let mut query_chars = query.chars();
    let mut current = query_chars.next()?;
    let mut score = 200;
    let mut matched = 0usize;
    let mut prev_index = None;

    for (i, ch) in command.chars().enumerate() {
        if ch != current {
            continue;
        }

        matched += 1;
        if let Some(prev) = prev_index {
            if i == prev + 1 {
                score += 8;
            } else {
                score -= (i - prev) as i32;
            }
        }
        prev_index = Some(i);

        if let Some(next) = query_chars.next() {
            current = next;
        } else {
            score -= (command.len() as i32 - matched as i32).max(0);
            return Some(score);
        }
    }

    None
}

fn command_aliases(command: &str) -> &'static [&'static str] {
    match command {
        "exit" => &["quit"],
        _ => &[],
    }
}
