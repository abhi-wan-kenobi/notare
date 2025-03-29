fn main() {
    let raw: serde_json::Value =
        serde_json::from_str(include_str!("assets/onboarding.raw.json")).unwrap();

    let words = raw["results"]["channels"][0]["alternatives"][0]["words"].clone();

    let diarizations: Vec<hypr_listener_interface::DiarizationChunk> = words
        .as_array()
        .unwrap()
        .iter()
        .map(|v| hypr_listener_interface::DiarizationChunk {
            start: (v["start"].as_f64().unwrap() * 1000.0) as u64,
            end: (v["end"].as_f64().unwrap() * 1000.0) as u64,
            speaker: v["speaker"].as_u64().unwrap().to_string(),
        })
        .collect();

    let transcripts: Vec<hypr_listener_interface::TranscriptChunk> = words
        .as_array()
        .unwrap()
        .iter()
        .map(|v| hypr_listener_interface::TranscriptChunk {
            text: v["word"].as_str().unwrap().to_string(),
            start: (v["start"].as_f64().unwrap() * 1000.0) as u64,
            end: (v["end"].as_f64().unwrap() * 1000.0) as u64,
        })
        .collect();

    #[derive(serde::Serialize)]
    struct Out {
        transcripts: Vec<hypr_listener_interface::TranscriptChunk>,
        diarizations: Vec<hypr_listener_interface::DiarizationChunk>,
    }

    std::fs::write(
        "assets/onboarding.processed.json",
        serde_json::to_string_pretty(&Out {
            transcripts,
            diarizations,
        })
        .unwrap(),
    )
    .unwrap();
}
