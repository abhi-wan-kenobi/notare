use crate::{
    entry::{EntryAction, EntryApp},
    event::{EventHandler, TuiEvent},
    frame::FrameRequester,
};

fn setup_panic_hook() {
    let original = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        ratatui::restore();
        original(info);
    }));
}

pub async fn run() -> EntryAction {
    setup_panic_hook();

    let mut terminal = ratatui::init();
    let (draw_tx, draw_rx) = tokio::sync::broadcast::channel(16);
    let frame_requester = FrameRequester::new(draw_tx);
    let mut app = EntryApp::new(frame_requester.clone());
    let mut events = EventHandler::new(draw_rx);
    events.resume_events();

    frame_requester.schedule_frame();

    loop {
        tokio::select! {
            Some(tui_event) = events.next() => {
                match tui_event {
                    TuiEvent::Key(key) => app.handle_key(key),
                    TuiEvent::Paste(pasted) => app.handle_paste(pasted),
                    TuiEvent::Draw => {
                        terminal.draw(|frame| crate::entry_ui::draw(frame, &mut app)).ok();
                    }
                }
            }
            else => break,
        }

        if app.should_quit {
            break;
        }
    }

    events.pause_events();
    ratatui::restore();

    app.action().unwrap_or(EntryAction::Quit)
}
