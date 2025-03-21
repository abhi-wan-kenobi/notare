use block2::RcBlock;
use objc2::runtime::Bool;
use objc2_avf_audio::AVAudioApplication;

pub fn main() -> std::sync::mpsc::Receiver<bool> {
    let instance = unsafe { AVAudioApplication::sharedInstance() };
    let (tx, rx) = std::sync::mpsc::channel::<bool>();

    let handler = RcBlock::new(move |v: Bool| -> Bool {
        let result = tx.send(v.as_bool());
        Bool::new(result.is_ok())
    });

    unsafe {
        instance
            .setInputMuteStateChangeHandler_error(Some(&handler))
            .unwrap();
    }

    rx
}
