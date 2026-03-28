use std::collections::VecDeque;

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{StatefulWidget, Widget};

const BLOCKS: [char; 9] = [' ', '▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];
pub const MIC_COLOR: Color = Color::Cyan;
pub const SYS_COLOR: Color = Color::Magenta;

fn to_perceptual(level: f32) -> f32 {
    if level <= 0.0 {
        0.0
    } else {
        let db = 20.0 * level.log10();
        ((db + 48.0) / 48.0).clamp(0.0, 1.0)
    }
}

fn block_char(perceptual: f32) -> char {
    let idx = (perceptual * 8.0).round() as usize;
    BLOCKS[idx.min(8)]
}

#[derive(Clone, Copy)]
pub enum WaveformMode {
    Mono,
    Dual,
}

pub struct LiveWaveformState {
    left: VecDeque<f32>,
    right: VecDeque<f32>,
    width: usize,
}

impl LiveWaveformState {
    pub fn new(width: usize) -> Self {
        Self {
            left: VecDeque::with_capacity(width + 1),
            right: VecDeque::with_capacity(width + 1),
            width,
        }
    }

    pub fn push(&mut self, left: f32, right: f32) {
        push_level(&mut self.left, left, self.width);
        push_level(&mut self.right, right, self.width);
    }
}

pub struct LiveWaveform {
    pub mode: WaveformMode,
}

impl StatefulWidget for LiveWaveform {
    type State = LiveWaveformState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        let width = area.width as usize;
        let spans = match self.mode {
            WaveformMode::Mono => mono_spans(&state.left, MIC_COLOR, width),
            WaveformMode::Dual => overlaid_spans(&state.left, &state.right, width),
        };
        let line = Line::from(spans);
        buf.set_line(area.x, area.y, &line, area.width);
    }
}

impl LiveWaveform {
    pub fn spans(
        state: &LiveWaveformState,
        mode: WaveformMode,
        width: usize,
    ) -> Vec<Span<'static>> {
        match mode {
            WaveformMode::Mono => mono_spans(&state.left, MIC_COLOR, width),
            WaveformMode::Dual => overlaid_spans(&state.left, &state.right, width),
        }
    }
}

pub struct PlaybackWaveform<'a> {
    pub peaks: &'a [f32],
    pub fraction: f64,
    pub color: Color,
}

impl Widget for PlaybackWaveform<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let width = area.width as usize;
        let spans = playback_spans(self.peaks, self.fraction, self.color, width);
        let line = Line::from(spans);
        buf.set_line(area.x, area.y, &line, area.width);
    }
}

impl<'a> PlaybackWaveform<'a> {
    pub fn spans(peaks: &[f32], fraction: f64, color: Color, width: usize) -> Vec<Span<'static>> {
        playback_spans(peaks, fraction, color, width)
    }
}

/// Compute peak amplitudes for `width` segments from f32 samples (range -1.0..1.0).
pub fn compute_peaks(samples: &[f32], width: usize) -> Vec<f32> {
    if samples.is_empty() || width == 0 {
        return vec![0.0; width];
    }
    let chunk_size = (samples.len() + width - 1) / width;
    let mut peaks = Vec::with_capacity(width);
    for chunk in samples.chunks(chunk_size) {
        let peak = chunk
            .iter()
            .map(|s| s.abs())
            .fold(0.0_f32, f32::max)
            .clamp(0.0, 1.0);
        peaks.push(peak);
    }
    peaks.truncate(width);
    while peaks.len() < width {
        peaks.push(0.0);
    }
    peaks
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

fn push_level(history: &mut VecDeque<f32>, level: f32, width: usize) {
    if history.len() >= width {
        history.pop_front();
    }
    history.push_back(level);
}

fn mono_spans(history: &VecDeque<f32>, color: Color, width: usize) -> Vec<Span<'static>> {
    let mut spans = Vec::with_capacity(width);
    let start = width.saturating_sub(history.len());
    for i in 0..width {
        if i < start {
            spans.push(Span::raw(" "));
        } else {
            let level = to_perceptual(history[i - start]);
            let ch = block_char(level);
            spans.push(Span::styled(String::from(ch), Style::default().fg(color)));
        }
    }
    spans
}

fn overlaid_spans(mic: &VecDeque<f32>, sys: &VecDeque<f32>, width: usize) -> Vec<Span<'static>> {
    let mut spans = Vec::with_capacity(width + 4);
    spans.push(Span::styled("mic", Style::default().fg(MIC_COLOR)));
    spans.push(Span::raw("/"));
    spans.push(Span::styled("sys", Style::default().fg(SYS_COLOR)));
    spans.push(Span::raw(" "));

    let mic_start = width.saturating_sub(mic.len());
    let sys_start = width.saturating_sub(sys.len());

    for i in 0..width {
        let m = if i >= mic_start {
            to_perceptual(mic[i - mic_start])
        } else {
            0.0
        };
        let s = if i >= sys_start {
            to_perceptual(sys[i - sys_start])
        } else {
            0.0
        };

        let level = m.max(s);
        if level <= 0.0 {
            spans.push(Span::raw(" "));
        } else {
            let color = if m >= s { MIC_COLOR } else { SYS_COLOR };
            spans.push(Span::styled(
                String::from(block_char(level)),
                Style::default().fg(color),
            ));
        }
    }
    spans
}

fn playback_spans(peaks: &[f32], fraction: f64, color: Color, width: usize) -> Vec<Span<'static>> {
    let resampled = resample_peaks(peaks, width);
    let max_peak = resampled.iter().cloned().fold(0.0_f32, f32::max);
    let norm = if max_peak > 0.0 { 1.0 / max_peak } else { 1.0 };
    let played_cols = (fraction * width as f64).round() as usize;
    let mut spans = Vec::with_capacity(width);
    for (i, &peak) in resampled.iter().enumerate() {
        let normalized = (peak * norm).clamp(0.0, 1.0);
        let ch = block_char(normalized);
        let fg = if i < played_cols {
            color
        } else {
            Color::DarkGray
        };
        spans.push(Span::styled(String::from(ch), Style::default().fg(fg)));
    }
    spans
}

fn resample_peaks(peaks: &[f32], width: usize) -> Vec<f32> {
    if peaks.is_empty() || width == 0 {
        return vec![0.0; width];
    }
    if peaks.len() == width {
        return peaks.to_vec();
    }
    let mut out = Vec::with_capacity(width);
    for i in 0..width {
        let pos = i as f64 * (peaks.len() as f64 - 1.0) / (width as f64 - 1.0).max(1.0);
        let lo = pos.floor() as usize;
        let hi = (lo + 1).min(peaks.len() - 1);
        let t = pos - lo as f64;
        out.push(peaks[lo] * (1.0 - t as f32) + peaks[hi] * t as f32);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compute_peaks_basic() {
        let samples: Vec<f32> = vec![0.03, -0.06, 0.09, -0.12, 0.15, -0.18, 0.21, -0.24];
        let peaks = compute_peaks(&samples, 4);
        assert_eq!(peaks.len(), 4);
        assert!(peaks[3] > peaks[0]);
    }

    #[test]
    fn compute_peaks_empty() {
        assert_eq!(compute_peaks(&[], 4), vec![0.0; 4]);
    }

    #[test]
    fn live_waveform_state_push() {
        let mut state = LiveWaveformState::new(4);
        for i in 0..6 {
            state.push(i as f32 * 0.1, 0.0);
        }
        assert_eq!(state.left.len(), 4);
        assert_eq!(state.right.len(), 4);
    }

    #[test]
    fn playback_spans_split_color() {
        let peaks = vec![0.5; 10];
        let spans = playback_spans(&peaks, 0.5, MIC_COLOR, 10);
        assert_eq!(spans.len(), 10);
        assert_eq!(spans[0].style.fg, Some(MIC_COLOR));
        assert_eq!(spans[9].style.fg, Some(Color::DarkGray));
    }

    #[test]
    fn overlaid_mic_dominant() {
        let mut mic = VecDeque::new();
        let mut sys = VecDeque::new();
        mic.push_back(0.5);
        sys.push_back(0.01);
        let spans = overlaid_spans(&mic, &sys, 1);
        let block_span = spans
            .iter()
            .find(|s| s.content.chars().any(|c| BLOCKS[1..].contains(&c)));
        assert!(block_span.is_some());
        assert_eq!(block_span.unwrap().style.fg, Some(MIC_COLOR));
    }

    #[test]
    fn overlaid_sys_dominant() {
        let mut mic = VecDeque::new();
        let mut sys = VecDeque::new();
        mic.push_back(0.01);
        sys.push_back(0.5);
        let spans = overlaid_spans(&mic, &sys, 1);
        let block_span = spans
            .iter()
            .find(|s| s.content.chars().any(|c| BLOCKS[1..].contains(&c)));
        assert!(block_span.is_some());
        assert_eq!(block_span.unwrap().style.fg, Some(SYS_COLOR));
    }
}
