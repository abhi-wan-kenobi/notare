use std::{
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

pub(crate) fn maybe_schedule_legacy_bundle_rename_on_launch<R: tauri::Runtime>(
    _app: &tauri::AppHandle<R>,
) -> Result<bool, crate::Error> {
    let current_app_path = current_app_bundle_path()?;
    let Some(target_app_path) = legacy_target_app_path(&current_app_path) else {
        return Ok(false);
    };
    if target_app_path.exists() {
        tracing::warn!(
            current_app_path = %current_app_path.display(),
            target_app_path = %target_app_path.display(),
            "skipping legacy macOS bundle rename because target already exists"
        );
        return Ok(false);
    }

    let mut command =
        build_bundle_rename_command(std::process::id(), &current_app_path, &target_app_path);
    command.stdin(Stdio::null());
    command.stdout(Stdio::null());
    command.stderr(Stdio::null());
    command
        .spawn()
        .map_err(|err| crate::Error::FailedToScheduleInstalledAppLaunch {
            path: target_app_path.display().to_string(),
            details: err.to_string(),
        })?;

    tracing::info!(
        current_app_path = %current_app_path.display(),
        target_app_path = %target_app_path.display(),
        "scheduled legacy macOS bundle rename on launch"
    );

    Ok(true)
}

fn legacy_target_app_path(current_app_path: &Path) -> Option<PathBuf> {
    let target_name = match current_app_path.file_name().and_then(|name| name.to_str()) {
        Some("Hyprnote.app") => "Char.app",
        Some("Hyprnote Nightly.app") => "Char Nightly.app",
        Some("Hyprnote Staging.app") => "Char Staging.app",
        _ => return None,
    };

    current_app_path
        .parent()
        .map(|parent| parent.join(target_name))
}

fn current_app_bundle_path() -> Result<PathBuf, crate::Error> {
    let executable_path = tauri::utils::platform::current_exe()?;
    current_app_bundle_path_from_executable(&executable_path)
}

fn current_app_bundle_path_from_executable(
    executable_path: &Path,
) -> Result<PathBuf, crate::Error> {
    let app_path = executable_path
        .parent()
        .and_then(Path::parent)
        .and_then(Path::parent)
        .ok_or(crate::Error::FailedToDetermineCurrentAppPath)?;

    if app_path.extension().and_then(|ext| ext.to_str()) != Some("app") {
        return Err(crate::Error::FailedToDetermineCurrentAppPath);
    }

    Ok(app_path.to_path_buf())
}

fn build_bundle_rename_command(
    current_pid: u32,
    current_app_path: &Path,
    target_app_path: &Path,
) -> Command {
    let script = format!(
        r#"while kill -0 "$1" 2>/dev/null; do sleep 0.1; done;
if [ -e {target} ]; then
  exit 1
fi

rename_app() {{
  if ! mv -f {current} {target}; then
    return 1
  fi
}}

if ! rename_app; then
  osascript -e {authorization} || exit 1
fi

touch {target} >/dev/null 2>&1 || true
open -n {target}"#,
        current = shell_quote(current_app_path),
        target = shell_quote(target_app_path),
        authorization = do_shell_script_with_privileges(&authorization_script(
            current_app_path,
            target_app_path,
        )),
    );

    let mut command = Command::new("/bin/sh");
    command
        .arg("-c")
        .arg(script)
        .arg("sh")
        .arg(current_pid.to_string());
    command
}

fn authorization_script(current_app_path: &Path, target_app_path: &Path) -> String {
    format!(
        "set -e; \
         if [ -e {target} ]; then exit 1; fi; \
         mv -f {current} {target}",
        current = shell_quote(current_app_path),
        target = shell_quote(target_app_path),
    )
}

fn shell_quote(path: &Path) -> String {
    let path = path.display().to_string().replace('\'', "'\"'\"'");
    format!("'{path}'")
}

fn do_shell_script_with_privileges(shell_script: &str) -> String {
    let escaped = shell_script.replace('\\', "\\\\").replace('"', "\\\"");
    format!(
        "'do shell script \"{}\" with administrator privileges'",
        escaped
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_legacy_bundle_names_to_char_names() {
        let cases = [
            ("/Applications/Hyprnote.app", "/Applications/Char.app"),
            (
                "/Applications/Hyprnote Nightly.app",
                "/Applications/Char Nightly.app",
            ),
            (
                "/Applications/Hyprnote Staging.app",
                "/Applications/Char Staging.app",
            ),
        ];

        for (current, expected) in cases {
            assert_eq!(
                legacy_target_app_path(Path::new(current)),
                Some(PathBuf::from(expected))
            );
        }
    }

    #[test]
    fn ignores_non_legacy_bundle_names() {
        assert_eq!(
            legacy_target_app_path(Path::new("/Applications/Char Nightly.app")),
            None
        );
    }

    #[test]
    fn rename_command_does_not_open_existing_target_bundle() {
        let command = build_bundle_rename_command(
            4242,
            Path::new("/Applications/Hyprnote Nightly.app"),
            Path::new("/Applications/Char Nightly.app"),
        );
        let args = command
            .get_args()
            .map(|arg| arg.to_string_lossy().to_string())
            .collect::<Vec<_>>();

        assert!(args[1].contains("if [ -e '/Applications/Char Nightly.app' ]; then"));
        assert!(!args[1].contains("open -n '/Applications/Char Nightly.app'\n  exit 0"));
    }

    #[test]
    fn current_bundle_path_from_executable_uses_bundle_root() {
        let executable = Path::new("/Applications/Char.app/Contents/MacOS/char");

        let bundle = current_app_bundle_path_from_executable(executable).unwrap();

        assert_eq!(bundle, PathBuf::from("/Applications/Char.app"));
    }

    #[test]
    fn rename_command_relaunches_from_target_bundle() {
        let command = build_bundle_rename_command(
            4242,
            Path::new("/Applications/Hyprnote Nightly.app"),
            Path::new("/Applications/Char Nightly.app"),
        );
        let args = command
            .get_args()
            .map(|arg| arg.to_string_lossy().to_string())
            .collect::<Vec<_>>();

        assert_eq!(command.get_program(), "/bin/sh");
        assert_eq!(args[0], "-c");
        assert!(args[1].contains(r#"while kill -0 "$1" 2>/dev/null; do sleep 0.1; done;"#));
        assert!(args[1].contains(
            "mv -f '/Applications/Hyprnote Nightly.app' '/Applications/Char Nightly.app'"
        ));
        assert!(args[1].contains("open -n '/Applications/Char Nightly.app'"));
        assert_eq!(&args[2..], ["sh", "4242"]);
    }
}
