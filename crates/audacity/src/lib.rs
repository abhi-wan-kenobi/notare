use std::fs;
use std::io;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct Track {
    path: PathBuf,
    name: Option<String>,
    muted: bool,
}

impl Track {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            name: None,
            muted: false,
        }
    }

    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    pub fn muted(mut self, muted: bool) -> Self {
        self.muted = muted;
        self
    }
}

#[derive(Debug, Clone, Default)]
pub struct Project {
    tracks: Vec<Track>,
    align_start_to_zero: bool,
}

impl Project {
    pub fn new() -> Self {
        Self {
            tracks: Vec::new(),
            align_start_to_zero: true,
        }
    }

    pub fn with_track(mut self, track: Track) -> Self {
        self.tracks.push(track);
        self
    }

    pub fn with_align_start_to_zero(mut self, align_start_to_zero: bool) -> Self {
        self.align_start_to_zero = align_start_to_zero;
        self
    }

    pub fn write_bundle(&self, out_dir: impl AsRef<Path>) -> io::Result<Bundle> {
        let out_dir = out_dir.as_ref();
        fs::create_dir_all(out_dir)?;

        let commands_path = out_dir.join("audacity_commands.txt");
        let script_path = out_dir.join("open_in_audacity.py");

        fs::write(&commands_path, self.render_commands()?)?;
        fs::write(&script_path, PYTHON_HELPER)?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;

            let mut permissions = fs::metadata(&script_path)?.permissions();
            permissions.set_mode(0o755);
            fs::set_permissions(&script_path, permissions)?;
        }

        Ok(Bundle {
            commands_path,
            script_path,
        })
    }

    fn render_commands(&self) -> io::Result<String> {
        let mut commands = Vec::new();

        for track in &self.tracks {
            commands.push(format!(
                "Import2: Filename={}",
                audacity_string(&track.path)?
            ));
        }

        for (index, track) in self.tracks.iter().enumerate() {
            commands.push(format!("SelectTracks: Track={index} TrackCount=1 Mode=Set"));
            if let Some(name) = &track.name {
                commands.push(format!("SetTrackStatus: Name={}", audacity_value(name)));
            }
            if track.muted {
                commands.push("SetTrackAudio: Mute=1".to_string());
            }
        }

        if self.align_start_to_zero && !self.tracks.is_empty() {
            commands.push(format!(
                "SelectTracks: Track=0 TrackCount={} Mode=Set",
                self.tracks.len()
            ));
            commands.push("Align_StartToZero:".to_string());
        }

        Ok(format!("{}\n", commands.join("\n")))
    }
}

#[derive(Debug, Clone)]
pub struct Bundle {
    pub commands_path: PathBuf,
    pub script_path: PathBuf,
}

fn audacity_string(path: &Path) -> io::Result<String> {
    let path = path.canonicalize()?;
    let path = path
        .to_string_lossy()
        .replace('\\', "\\\\")
        .replace('"', "\\\"");
    Ok(format!("\"{path}\""))
}

fn audacity_value(value: &str) -> String {
    format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
}

const PYTHON_HELPER: &str = r##"#!/usr/bin/env python3
import os
import queue
import signal
import threading
import time
from pathlib import Path


def find_pipe_paths():
    env_to = os.environ.get("AUDACITY_PIPE_TO")
    env_from = os.environ.get("AUDACITY_PIPE_FROM")
    if env_to and env_from:
        return Path(env_to), Path(env_from)

    uid = os.getuid()
    candidates = [
        (Path(f"/tmp/audacity_script_pipe.to.{uid}"), Path(f"/tmp/audacity_script_pipe.from.{uid}")),
        (Path("/tmp/audacity_script_pipe.to"), Path("/tmp/audacity_script_pipe.from")),
    ]

    for to_path, from_path in candidates:
        if to_path.exists() and from_path.exists():
            return to_path, from_path

    raise SystemExit(
        "Audacity pipe not found.\n"
        "1. Open Audacity.\n"
        "2. Go to Audacity > Preferences > Modules.\n"
        "3. Set mod-script-pipe to Enabled.\n"
        "4. Restart Audacity.\n"
        "5. Verify the pipes exist with:\n"
        "   find /tmp /private/tmp -maxdepth 1 -name 'audacity_script_pipe*'\n"
        "6. If Audacity uses non-default names, set AUDACITY_PIPE_TO and "
        "AUDACITY_PIPE_FROM before rerunning this helper."
    )


def open_pipe(path, mode, timeout=3.0):
    result = queue.Queue(maxsize=1)

    def worker():
        try:
            pipe = path.open(mode, encoding="utf-8", errors="replace")
        except Exception as exc:
            result.put(exc)
            return
        result.put(pipe)

    thread = threading.Thread(target=worker, daemon=True)
    thread.start()
    thread.join(timeout)
    if thread.is_alive():
        raise SystemExit(
            f"Timed out opening {path}. Make sure Audacity is running and mod-script-pipe is enabled."
        )

    opened = result.get()
    if isinstance(opened, Exception):
        raise SystemExit(f"Failed to open {path}: {opened}")
    return opened


def drain_responses(from_pipe, done):
    while not done.is_set():
        line = from_pipe.readline()
        if line == "":
            done.set()
            return

        line = line.rstrip("\r\n")
        if line:
            print(f"<- {line}")


def exit_now(code):
    os._exit(code)


def main():
    command_path = Path(__file__).with_name("audacity_commands.txt")
    if not command_path.exists():
        raise SystemExit(f"missing command file: {command_path}")

    to_path, from_path = find_pipe_paths()
    commands = [
        line.strip()
        for line in command_path.read_text(encoding="utf-8").splitlines()
        if line.strip() and not line.lstrip().startswith("#")
    ]

    done = threading.Event()
    signal.signal(signal.SIGINT, lambda *_: exit_now(130))

    with open_pipe(to_path, "w") as to_pipe, open_pipe(from_path, "r") as from_pipe:
        reader = threading.Thread(target=drain_responses, args=(from_pipe, done), daemon=True)
        reader.start()

        for command in commands:
            print(f"-> {command}")
            to_pipe.write(command + "\n")
            to_pipe.flush()
            time.sleep(0.3)

        time.sleep(1.0)
        done.set()
        time.sleep(0.2)
        exit_now(0)


if __name__ == "__main__":
    try:
        main()
    except BrokenPipeError:
        raise SystemExit("Audacity pipe closed while sending commands.")
"##;
