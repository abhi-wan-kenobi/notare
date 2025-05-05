use intervaltree::IntervalTree;

use hypr_listener_interface::{DiarizationChunk, TranscriptChunk};

#[macro_export]
macro_rules! common_derives {
    ($item:item) => {
        #[derive(
            Debug, Default, Clone, PartialEq, serde::Serialize, serde::Deserialize, specta::Type,
        )]
        $item
    };
}

pub trait Interval {
    fn start(&self) -> u64;
    fn end(&self) -> u64;

    fn overlaps<I: Interval>(&self, other: &I) -> Option<u64> {
        if self.start() < other.end() && self.end() > other.start() {
            let overlap_start = std::cmp::max(self.start(), other.start());
            let overlap_end = std::cmp::min(self.end(), other.end());
            Some(overlap_end - overlap_start)
        } else {
            None
        }
    }
}

impl Interval for DiarizationChunk {
    fn start(&self) -> u64 {
        self.start
    }
    fn end(&self) -> u64 {
        self.end
    }
}

impl Interval for TranscriptChunk {
    fn start(&self) -> u64 {
        self.start
    }
    fn end(&self) -> u64 {
        self.end
    }
}

impl Interval for std::ops::Range<u64> {
    fn start(&self) -> u64 {
        self.start
    }
    fn end(&self) -> u64 {
        self.end
    }
}

common_derives! {
    pub struct Timeline {
        transcripts: Vec<TranscriptChunk>,
        diarizations: Vec<DiarizationChunk>,
        max_confidence: f32,
    }
}

common_derives! {
    pub struct TimelineView {
        pub items: Vec<TimelineViewItem>,
    }
}

common_derives! {
    pub struct TimelineViewItem {
        pub start: u64,
        pub end: u64,
        pub speaker: i32,
        pub text: String,
        pub confidence: f32,
    }
}

common_derives! {
    pub struct TimelineFilter {
        pub last_n_seconds: Option<u64>,
    }
}

impl std::fmt::Display for TimelineView {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for item in self.items.iter() {
            writeln!(f, "{}", item)?;
        }
        Ok(())
    }
}

impl std::fmt::Display for TimelineViewItem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}\n{}\n", self.speaker, self.text)
    }
}

impl TimelineViewItem {
    fn eos(&self) -> bool {
        matches!(
            self.text.chars().last().unwrap(),
            '.' | '?' | '!' | '？' | '！'
        )
    }

    fn num_words(&self) -> usize {
        self.text.split_whitespace().count()
    }

    fn time_diff(&self, other: &TranscriptChunk) -> u64 {
        self.end.abs_diff(other.start)
    }

    fn merge(&mut self, other: &TranscriptChunk) {
        self.end = other.end;

        self.text.push(' ');
        self.text.push_str(&other.text);
    }
}

impl Timeline {
    pub fn add_transcription(&mut self, item: TranscriptChunk) {
        if let Some(confidence) = item.confidence {
            if confidence > self.max_confidence || self.max_confidence == 0.0 {
                self.max_confidence = confidence;
            }
        }

        if !self.transcripts.is_empty() {
            let last = self.transcripts.last().unwrap();

            if item.start <= last.end + 500
                && !last.text.ends_with(|c| matches!(c, '.' | '?' | '!' | '！'))
            {
                let mut merged = last.clone();
                merged.end = item.end;

                merged.text.push_str(" ");
                merged.text.push_str(&item.text);

                *self.transcripts.last_mut().unwrap() = merged;
                return;
            }
        }

        self.transcripts.push(item);
    }

    pub fn add_diarization(&mut self, item: DiarizationChunk) {
        if let Some(index) = self
            .diarizations
            .iter()
            .position(|x| x.start() == item.start())
        {
            self.diarizations[index] = item;
        } else {
            self.diarizations.push(item);
        }
    }

    pub fn view(&self, filter: TimelineFilter) -> TimelineView {
        let tree: IntervalTree<u64, i32> = IntervalTree::from_iter(
            self.diarizations
                .iter()
                .map(|d| (d.start..d.end, d.speaker.clone())),
        );

        let mut items: Vec<TimelineViewItem> = vec![];

        let max_end = self.transcripts.iter().map(|t| t.end).max().unwrap_or(0);
        let filtered_transcripts = self.transcripts.iter().filter(|t| {
            filter
                .last_n_seconds
                .is_none_or(|n| t.end >= max_end.saturating_sub(n * 1000))
        });

        let mut streaming_mode = false;
        if self.transcripts.len() > 5 {
            let avg_length = self.transcripts.iter().map(|t| t.text.len()).sum::<usize>() as f32
                / self.transcripts.len() as f32;

            streaming_mode = avg_length < 10.0;
        }

        for transcript in filtered_transcripts {
            let range = transcript.start.saturating_sub(100)..transcript.end.saturating_add(100);
            let speakers: Vec<_> = tree.query(range).collect();

            let normalized_confidence = transcript
                .confidence
                .map(|conf| {
                    if self.max_confidence > 0.0 {
                        conf / self.max_confidence
                    } else {
                        1.0
                    }
                })
                .unwrap_or(1.0);

            if speakers.is_empty() {
                if streaming_mode && !items.is_empty() {
                    let last_item = items.last_mut().unwrap();
                    if transcript.start <= last_item.end + 800 {
                        last_item.merge(transcript);
                        continue;
                    }
                }

                items.push(TimelineViewItem {
                    start: transcript.start,
                    end: transcript.end,
                    speaker: -1,
                    text: transcript.text.clone(),
                    confidence: normalized_confidence,
                });

                continue;
            }

            let speaker = speakers
                .iter()
                .map(|entry| {
                    let diarization_interval = entry.range.start..entry.range.end;
                    let overlap = transcript.overlaps(&diarization_interval).unwrap_or(0);

                    (entry.value.clone(), overlap)
                })
                .max_by_key(|(_, overlap)| *overlap)
                .map(|(speaker, _)| speaker)
                .unwrap();

            if let Some(last_item) = items.last_mut() {
                if transcript.text.trim().is_empty() {
                    last_item.merge(transcript);
                    continue;
                }

                if last_item.speaker == speaker {
                    if streaming_mode && transcript.start <= last_item.end + 800 {
                        last_item.merge(transcript);
                        continue;
                    }

                    if last_item.num_words() < 5 && last_item.time_diff(transcript) < 5000 {
                        last_item.merge(transcript);
                        continue;
                    }

                    if !last_item.eos() && last_item.time_diff(transcript) < 2000 {
                        last_item.merge(transcript);
                        continue;
                    }

                    if last_item.eos()
                        && last_item.time_diff(transcript) < 500
                        && last_item.num_words() <= 20
                    {
                        last_item.merge(transcript);
                        continue;
                    }
                }
            }

            items.push(TimelineViewItem {
                start: transcript.start,
                end: transcript.end,
                speaker,
                text: transcript.text.clone(),
                confidence: normalized_confidence,
            });
        }

        TimelineView { items }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(test)]
    macro_rules! init_timeline {
        ($module:ident) => {{
            let transcripts: Vec<TranscriptChunk> =
                serde_json::from_str(hypr_data::$module::TRANSCRIPTION_JSON).unwrap();
            let diarizations: Vec<DiarizationChunk> =
                serde_json::from_str(hypr_data::$module::DIARIZATION_JSON).unwrap();

            let mut timeline = Timeline::default();
            for t in transcripts {
                timeline.add_transcription(t);
            }
            for d in diarizations {
                timeline.add_diarization(d);
            }
            timeline
        }};
    }

    #[test]
    fn test_english_3() {
        let timeline = init_timeline!(english_3);

        insta::assert_snapshot!(timeline.view(TimelineFilter::default()).to_string(), @r###"
        0
        -Okay. Michael, why don't you start us off?

        1
        -That wasn't much of an introduction. -Ladies and gentlemen, your boss, Michael Scott. Still lame. Okay. Alright. Thank you, Ryan, for that wonderful introduction.

        1
        Okay. Today we're going to be talking about PowerPoint. PowerPoint. PowerPoint. PowerPoint. Yes, I forgot about Ryan's presentation and yes, it would have been nice to do well with the first presentation that he'd given me. But you know what else would have been nice? Winning the lottery. And the best way to start is to hit start, and up comes the toolbar. That's what she said.

        1
        What we have to do here is go to run, and then you look up to PowerPoint, and we are in. We are going to register. You hit register. Updates are ready. I should update.

        1
        Estimated time twelve minutes, so this should take about five or ten minutes.

        0
        This is the first time you've opened PowerPoint. Why? -You didn't prepare a presentation at all, did you?

        2
        Know what?

        1
        I had a really rough night, and my boss can back me up on that.

        0
        -I'm your boss. -My other boss, Mr. Figaro. -You have another job?

        1
        -What I do between 05:30 p. M. And one a. M. Is nobody's business but mine and my other businesses.

        0
        -Are you going to waitress? -You cannot have a second job if it affects your work here.

        1
        -It won't? -It did already. -Okay. Honestly, it is unlikely that I was gonna figure this out anyway.

        0
        That is so funny. Why is Daryl here? He works in a warehouse. I invited him. It's not a party. Daryl, back downstairs. This isn't the information you need.

        2
        This information here? Yeah. You're right. I don't need this. Okay.

        2
        Hey. Come on.

        0
        See you later tonight.

        2
        I got plans later.

        0
        Okay. Bye, honey. How long until you actually get this presentation ready?

        1
        Don't you do this presentation? Because I you know how to do it.

        0
        What I really want, honestly Michael, is for you to know it so that you can communicate it to the people here, to your clients, to whomever.

        1
        Okay. What? It's whoever not whomever. Not whomever. No whomever is never actually right.

        0
        Well sometimes it's right.

        1
        Michael is right. It's a made up word used to trick students.

        3
        No. Actually whomever is the formal version of the word.

        0
        Obviously it's a real word but I don't know when to use it correctly.

        1
        Not a native speaker.

        2
        I know what's right, but I'm not gonna say because you're all jerks who didn't come see my band last night.

        0
        Do you really know which one is correct?

        2
        I don't know.

        0
        It's whom when it's the object of the sentence and who when it's the subject. Subject. That sounds right. Well, sounds right but is it How did Ryan use it as an object? As an object. Ryan used me as an object.

        2
        Is he writing about the How did he use it again?

        3
        It was Ryan wanted Michael, the subject, to explain the computer system, the object.

        1
        Thank you.

        3
        To whomever, meaning us, the indirect object, which is the correct usage of the word.

        0
        No one asked you anything ever, so whomever's name is Toby, why don't you take a letter opener and stick it in your skull? Hey, this doesn't matter, and I don't even care. Michael, you quit the other job or you're fired here.
        "###);
    }

    #[test]
    fn test_streaming_transcription() {
        let mut timeline = Timeline::default();

        timeline.add_transcription(TranscriptChunk {
            start: 1000,
            end: 1500,
            text: "Fastest".to_string(),
            confidence: Some(0.9),
        });
        timeline.add_transcription(TranscriptChunk {
            start: 1500,
            end: 1800,
            text: " AI".to_string(),
            confidence: Some(0.9),
        });
        timeline.add_transcription(TranscriptChunk {
            start: 1800,
            end: 2000,
            text: " chat".to_string(),
            confidence: Some(0.9),
        });
        timeline.add_transcription(TranscriptChunk {
            start: 2000,
            end: 2400,
            text: " app".to_string(),
            confidence: Some(0.9),
        });

        timeline.add_transcription(TranscriptChunk {
            start: 3500,
            end: 4000,
            text: "It's really good.".to_string(),
            confidence: Some(0.9),
        });

        let view = timeline.view(TimelineFilter::default());

        assert_eq!(view.items.len(), 2);
        assert_eq!(view.items[0].text, "Fastest AI chat app");
        assert_eq!(view.items[1].text, "It's really good.");
    }
}
