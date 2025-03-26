// https://developer.apple.com/documentation/appkit/nsworkspace?language=objc
// https://developer.apple.com/documentation/foundation/nsdistributednotificationcenter/1414151-addobserver
// https://docs.rs/objc2-foundation/0.2.2/objc2_foundation/struct.NSNotificationCenter.html#method.addObserver_selector_name_object

// https://github.com/search?q=addObserver_forKeyPath_options_context+language:Rust+&type=code
// https://github.com/search?q=addObserver_selector_name_object+language:Rust+&type=code

use std::sync::{Arc, Mutex};

use block2::{Block, RcBlock};
use objc2::runtime::{Bool, ProtocolObject};
use objc2::{define_class, DefinedClass};
use objc2::{msg_send, MainThreadOnly};
use objc2_app_kit::NSWorkspace;
use objc2_foundation::{
    MainThreadMarker, NSArray, NSError, NSKeyValueObservingOptions, NSNotificationCenter, NSObject,
    NSObjectNSKeyValueObserverRegistration, NSObjectProtocol, NSString,
};
use objc2_user_notifications::{
    UNAuthorizationOptions, UNMutableNotificationContent, UNNotification, UNNotificationAction,
    UNNotificationActionOptions, UNNotificationPresentationOptions, UNNotificationRequest,
    UNNotificationResponse, UNTimeIntervalNotificationTrigger, UNUserNotificationCenter,
    UNUserNotificationCenterDelegate,
};

#[derive(Default)]
struct NotificationDelegateIvars {
    action_handler: Option<Arc<Mutex<dyn Fn(&str) + Send + 'static>>>,
}

define_class!(
    #[unsafe(super = NSObject)]
    #[thread_kind = MainThreadOnly]
    #[name = "NotificationDelegate"]
    #[ivars = NotificationDelegateIvars]
    struct NotificationDelegate;

    unsafe impl NSObjectProtocol for NotificationDelegate {}

    unsafe impl UNUserNotificationCenterDelegate for NotificationDelegate {
        #[unsafe(method(userNotificationCenter:didReceiveNotificationResponse:withCompletionHandler:))]
        fn user_notification_center_did_receive_notification_response_with_completion_handler(
            &self,
            _center: &UNUserNotificationCenter,
            response: &UNNotificationResponse,
            completion_handler: &Block<dyn Fn()>,
        ) {
            let action_identifier = unsafe { response.actionIdentifier() };
            if let Some(handler) = &self.ivars().action_handler {
                if let Ok(handler) = handler.lock() {
                    handler(&action_identifier.to_string());
                }
            }

            completion_handler.call(())
        }

        #[unsafe(method(userNotificationCenter:willPresentNotification:withCompletionHandler:))]
        fn user_notification_center_will_present_notification_with_completion_handler(
            &self,
            _center: &UNUserNotificationCenter,
            _notification: &UNNotification,
            completion_handler: &Block<dyn Fn(UNNotificationPresentationOptions)>,
        ) {
            let options = UNNotificationPresentationOptions::Banner
                | UNNotificationPresentationOptions::Sound;

            completion_handler.call((options,))
        }
    }
);

impl NotificationDelegate {
    fn new(mtm: MainThreadMarker) -> objc2::rc::Retained<Self> {
        let handler = Arc::new(Mutex::new(|action_id: &str| {
            println!("Received notification action: {}", action_id);
        }));

        let this = Self::alloc(mtm).set_ivars(NotificationDelegateIvars {
            action_handler: Some(handler),
        });
        unsafe { msg_send![super(this), init] }
    }
}

pub fn send_notification(title: &str, body: &str, actions: &[(&str, &str)]) {
    let _ = MainThreadMarker::new().unwrap();
    let un_center = unsafe { UNUserNotificationCenter::currentNotificationCenter() };

    let authorization_options = UNAuthorizationOptions::Alert
        | UNAuthorizationOptions::Sound
        | UNAuthorizationOptions::Badge;

    let (tx, rx) = std::sync::mpsc::channel::<bool>();
    let completion_handler = RcBlock::new(move |granted: Bool, _error: *mut NSError| {
        let _ = tx.send(granted.as_bool());
    });

    unsafe {
        un_center.requestAuthorizationWithOptions_completionHandler(
            authorization_options,
            &completion_handler,
        );
    }

    if let Ok(granted) = rx.recv() {
        if !granted {
            return;
        }
    }

    let content = unsafe { UNMutableNotificationContent::new() };
    let sound = unsafe { objc2_user_notifications::UNNotificationSound::defaultSound() };
    unsafe {
        content.setTitle(&NSString::from_str(title));
        content.setBody(&NSString::from_str(body));
        content.setSound(Some(&sound));
    }

    if !actions.is_empty() {
        let mut notification_actions = Vec::new();

        for (identifier, title) in actions {
            let action = unsafe {
                UNNotificationAction::actionWithIdentifier_title_options(
                    &NSString::from_str(identifier),
                    &NSString::from_str(title),
                    UNNotificationActionOptions::Foreground,
                )
            };
            notification_actions.push(action);
        }

        let _ = unsafe {
            let mut array = NSArray::from_retained_slice(&notification_actions);
            for action in &notification_actions {
                array = array.arrayByAddingObject(action);
            }
            array
        };
    }

    let trigger =
        unsafe { UNTimeIntervalNotificationTrigger::triggerWithTimeInterval_repeats(1.0, false) };

    let request_identifier = format!(
        "notification-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
    );

    let request = unsafe {
        UNNotificationRequest::requestWithIdentifier_content_trigger(
            &NSString::from_str(&request_identifier),
            &content,
            Some(&trigger),
        )
    };

    let (tx, rx) = std::sync::mpsc::channel::<bool>();
    let completion_handler = RcBlock::new(move |error: *mut NSError| {
        let success = error.is_null();
        let _ = tx.send(success);
    });

    unsafe {
        un_center.addNotificationRequest_withCompletionHandler(&request, Some(&completion_handler));
    }

    let _ = rx.recv();
}

fn setup_notification_center() -> objc2::rc::Retained<NotificationDelegate> {
    let mtm = MainThreadMarker::new().unwrap();

    let un = unsafe { UNUserNotificationCenter::currentNotificationCenter() };
    let delegate = NotificationDelegate::new(mtm);
    unsafe { un.setDelegate(Some(ProtocolObject::from_ref(&*delegate))) };

    delegate
}

pub fn run2() {
    let _delegate = setup_notification_center();

    // Example notification with actions
    send_notification(
        "Notification Title",
        "This is a test notification with actions",
        &[("accept", "Accept"), ("decline", "Decline")],
    );
}

pub fn run() {
    // https://developer.apple.com/documentation/appkit/nsworkspace/didlaunchapplicationnotification
    let workspace = unsafe { NSWorkspace::sharedWorkspace() };

    unsafe {
        let key = NSString::from_str("didLaunchApplication");
        workspace.addObserver_forKeyPath_options_context(
            // TODO
            &NSObject::new(),
            &key,
            NSKeyValueObservingOptions::empty(),
            std::ptr::null_mut(),
        );
    }

    let _nc = unsafe { NSNotificationCenter::defaultCenter() };
    // nc.addObserver_selector_name_object(observer, a_selector, a_name, an_object);

    let _un = unsafe { objc2_user_notifications::UNUserNotificationCenter::new() };
    // un.requestAuthorizationWithOptions_completionHandler(options, completion_handler);
}
