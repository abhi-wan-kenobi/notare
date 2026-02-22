use std::collections::VecDeque;
use std::io::{BufRead, BufReader};
use std::os::fd::FromRawFd;
use std::sync::{Arc, Mutex};

pub type LogBuffer = Arc<Mutex<VecDeque<String>>>;

const MAX_LINES: usize = 20;

pub fn setup() -> LogBuffer {
    let buffer: LogBuffer = Arc::new(Mutex::new(VecDeque::with_capacity(MAX_LINES)));
    let buffer_clone = buffer.clone();

    let mut pipe_fds = [-1i32; 2];
    // SAFETY: pipe(2) is safe to call; fds are valid for the lifetime of the process.
    unsafe {
        libc::pipe(pipe_fds.as_mut_ptr());
        libc::dup2(pipe_fds[1], libc::STDERR_FILENO);
        libc::close(pipe_fds[1]);
    }

    std::thread::Builder::new()
        .name("stderr-reader".into())
        .spawn(move || {
            let file = unsafe { std::fs::File::from_raw_fd(pipe_fds[0]) };
            let reader = BufReader::new(file);
            for line in reader.lines().map_while(Result::ok) {
                if line.is_empty() {
                    continue;
                }
                let mut buf = buffer_clone.lock().unwrap();
                if buf.len() >= MAX_LINES {
                    buf.pop_front();
                }
                buf.push_back(line);
            }
        })
        .expect("failed to spawn stderr-reader thread");

    buffer
}
