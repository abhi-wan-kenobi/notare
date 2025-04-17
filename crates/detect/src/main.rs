use cidre::{blocks, ns, ns::workspace::notification as wsn, objc::Obj};

const MEETING_APP_LIST: [&str; 1] = ["us.zoom.xos"];

fn main() {
    let block = |n: &ns::Notification| {
        let user_info = n.user_info().unwrap();

        if let Some(app) = user_info.get(wsn::app_key()) {
            if let Some(app) = app.try_cast(ns::RunningApp::cls()) {
                let bundle_id = app.bundle_id().unwrap().to_string();
                let detected = MEETING_APP_LIST.contains(&bundle_id.as_str());
                println!("{}: {}", bundle_id, detected);
            }
        }
    };

    let mut block = blocks::SyncBlock::new1(block);

    let notifications = [wsn::did_launch_app()];
    let mut observers = Vec::with_capacity(notifications.len());

    let mut nc = ns::Workspace::shared().notification_center();

    for name in notifications {
        observers.push(nc.add_observer_block(name, None, None, &mut block));
    }

    ns::App::shared().run();
}
