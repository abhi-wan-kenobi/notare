//! On-disk store for imported `.ics` files.
//!
//! Imported files are *copied* into an app-owned directory (so the source
//! file/USB stick/download can vanish) and tracked by an `index.json` next to
//! them. Each imported file is one calendar; the stored copy is re-parsed on
//! every refresh.
//!
//! Layout (inside the app data dir, e.g. `<data>/calendars/ics/`):
//! ```text
//! index.json          # Vec<IcsFileEntry>
//! <id>.ics            # verbatim copy of the imported file
//! ```

use std::path::{Path, PathBuf};

use crate::{Error, parse::parse_ics};

const INDEX_FILE: &str = "index.json";

/// Persisted per-file record.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct IcsFileEntry {
    id: String,
    /// Original file name at import time (e.g. `team-calendar.ics`).
    file_name: String,
    imported_at: String,
    updated_at: String,
}

/// Public listing item (entry + data derived from the stored copy).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct IcsFileInfo {
    pub id: String,
    pub file_name: String,
    /// `X-WR-CALNAME` from the file, when present.
    pub calendar_name: Option<String>,
    /// Display name: calendar name, else the file name without extension.
    pub title: String,
    pub event_count: u32,
    pub imported_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone)]
pub struct IcsStore {
    dir: PathBuf,
}

impl IcsStore {
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        Self { dir: dir.into() }
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// True when at least one file has been imported (cheap connected-check).
    pub fn has_files(&self) -> bool {
        self.load_index().map(|i| !i.is_empty()).unwrap_or(false)
    }

    pub fn list(&self) -> Result<Vec<IcsFileInfo>, Error> {
        let index = self.load_index()?;
        Ok(index.iter().map(|entry| self.describe(entry)).collect())
    }

    /// Validate + copy a `.ics` file into the store. Each import is a new
    /// calendar, even for the same source path.
    pub fn import(&self, source: &Path) -> Result<IcsFileInfo, Error> {
        let text = std::fs::read_to_string(source)?;
        parse_ics(&text)?; // reject files we cannot serve events from

        std::fs::create_dir_all(&self.dir)?;
        let id = uuid::Uuid::new_v4().to_string();
        std::fs::write(self.file_path(&id), &text)?;

        let now = now_rfc3339();
        let entry = IcsFileEntry {
            id,
            file_name: source
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| "calendar.ics".to_string()),
            imported_at: now.clone(),
            updated_at: now,
        };

        let mut index = self.load_index()?;
        index.push(entry.clone());
        self.save_index(&index)?;

        Ok(self.describe(&entry))
    }

    /// Replace the stored copy of an imported calendar with a fresh file
    /// (keeps the id, so enabled-state and synced events stay attached).
    pub fn replace(&self, id: &str, source: &Path) -> Result<IcsFileInfo, Error> {
        let mut index = self.load_index()?;
        let entry = index
            .iter_mut()
            .find(|entry| entry.id == id)
            .ok_or_else(|| Error::UnknownCalendar(id.to_string()))?;

        let text = std::fs::read_to_string(source)?;
        parse_ics(&text)?;

        std::fs::create_dir_all(&self.dir)?;
        std::fs::write(self.dir.join(format!("{id}.ics")), &text)?;

        entry.file_name = source
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| entry.file_name.clone());
        entry.updated_at = now_rfc3339();
        let described = self.describe(entry);
        self.save_index(&index)?;
        Ok(described)
    }

    pub fn remove(&self, id: &str) -> Result<(), Error> {
        let mut index = self.load_index()?;
        let before = index.len();
        index.retain(|entry| entry.id != id);
        if index.len() == before {
            return Err(Error::UnknownCalendar(id.to_string()));
        }
        self.save_index(&index)?;
        match std::fs::remove_file(self.file_path(id)) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(e.into()),
        }
    }

    /// Read + parse the stored copy of one imported calendar.
    pub fn read_calendar(&self, id: &str) -> Result<crate::parse::IcsCalendar, Error> {
        let index = self.load_index()?;
        if !index.iter().any(|entry| entry.id == id) {
            return Err(Error::UnknownCalendar(id.to_string()));
        }
        let text = std::fs::read_to_string(self.file_path(id))?;
        parse_ics(&text)
    }

    fn file_path(&self, id: &str) -> PathBuf {
        self.dir.join(format!("{id}.ics"))
    }

    fn describe(&self, entry: &IcsFileEntry) -> IcsFileInfo {
        let parsed = std::fs::read_to_string(self.file_path(&entry.id))
            .ok()
            .and_then(|text| parse_ics(&text).ok());

        let calendar_name = parsed.as_ref().and_then(|c| c.name.clone());
        let title = calendar_name.clone().unwrap_or_else(|| {
            entry
                .file_name
                .strip_suffix(".ics")
                .unwrap_or(&entry.file_name)
                .to_string()
        });

        IcsFileInfo {
            id: entry.id.clone(),
            file_name: entry.file_name.clone(),
            calendar_name,
            title,
            event_count: parsed.map(|c| c.events.len() as u32).unwrap_or(0),
            imported_at: entry.imported_at.clone(),
            updated_at: entry.updated_at.clone(),
        }
    }

    fn load_index(&self) -> Result<Vec<IcsFileEntry>, Error> {
        let path = self.dir.join(INDEX_FILE);
        match std::fs::read_to_string(&path) {
            Ok(text) => serde_json::from_str(&text).map_err(|e| Error::Index(e.to_string())),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
            Err(e) => Err(e.into()),
        }
    }

    fn save_index(&self, index: &[IcsFileEntry]) -> Result<(), Error> {
        std::fs::create_dir_all(&self.dir)?;
        let text =
            serde_json::to_string_pretty(index).map_err(|e| Error::Index(e.to_string()))?;
        std::fs::write(self.dir.join(INDEX_FILE), text)?;
        Ok(())
    }
}

fn now_rfc3339() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nX-WR-CALNAME:Team Calendar\r\nBEGIN:VEVENT\r\nUID:evt-1\r\nDTSTART:20260117T090000Z\r\nDTEND:20260117T100000Z\r\nSUMMARY:Standup\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n";
    const SAMPLE_UNNAMED: &str = "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nBEGIN:VEVENT\r\nUID:evt-2\r\nDTSTART:20260118T090000Z\r\nSUMMARY:Other\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n";

    fn write_source(dir: &Path, name: &str, contents: &str) -> PathBuf {
        let path = dir.join(name);
        std::fs::write(&path, contents).unwrap();
        path
    }

    #[test]
    fn import_copies_file_and_survives_source_deletion() {
        let tmp = tempfile::tempdir().unwrap();
        let store = IcsStore::new(tmp.path().join("store"));
        let source = write_source(tmp.path(), "team.ics", SAMPLE);

        let info = store.import(&source).unwrap();
        assert_eq!(info.file_name, "team.ics");
        assert_eq!(info.calendar_name.as_deref(), Some("Team Calendar"));
        assert_eq!(info.title, "Team Calendar");
        assert_eq!(info.event_count, 1);

        std::fs::remove_file(&source).unwrap();
        let listed = store.list().unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].id, info.id);
        assert_eq!(store.read_calendar(&info.id).unwrap().events.len(), 1);
    }

    #[test]
    fn title_falls_back_to_file_name_without_extension() {
        let tmp = tempfile::tempdir().unwrap();
        let store = IcsStore::new(tmp.path().join("store"));
        let source = write_source(tmp.path(), "personal-stuff.ics", SAMPLE_UNNAMED);
        let info = store.import(&source).unwrap();
        assert_eq!(info.calendar_name, None);
        assert_eq!(info.title, "personal-stuff");
    }

    #[test]
    fn import_rejects_malformed_files() {
        let tmp = tempfile::tempdir().unwrap();
        let store = IcsStore::new(tmp.path().join("store"));
        let source = write_source(tmp.path(), "broken.ics", "not an ics file");
        assert!(store.import(&source).is_err());
        assert!(!store.has_files());
    }

    #[test]
    fn replace_keeps_id_and_updates_content() {
        let tmp = tempfile::tempdir().unwrap();
        let store = IcsStore::new(tmp.path().join("store"));
        let source = write_source(tmp.path(), "team.ics", SAMPLE);
        let info = store.import(&source).unwrap();

        let updated = write_source(tmp.path(), "team-v2.ics", SAMPLE_UNNAMED);
        let replaced = store.replace(&info.id, &updated).unwrap();
        assert_eq!(replaced.id, info.id);
        assert_eq!(replaced.file_name, "team-v2.ics");
        assert_eq!(replaced.calendar_name, None);

        let calendar = store.read_calendar(&info.id).unwrap();
        assert_eq!(calendar.events[0].uid, "evt-2");

        // Replacing with a broken file keeps the old copy.
        let broken = write_source(tmp.path(), "broken.ics", "nope");
        assert!(store.replace(&info.id, &broken).is_err());
        assert_eq!(store.read_calendar(&info.id).unwrap().events[0].uid, "evt-2");
    }

    #[test]
    fn remove_deletes_entry_and_file() {
        let tmp = tempfile::tempdir().unwrap();
        let store = IcsStore::new(tmp.path().join("store"));
        let source = write_source(tmp.path(), "team.ics", SAMPLE);
        let info = store.import(&source).unwrap();
        assert!(store.has_files());

        store.remove(&info.id).unwrap();
        assert!(!store.has_files());
        assert!(store.read_calendar(&info.id).is_err());
        assert!(matches!(
            store.remove(&info.id),
            Err(Error::UnknownCalendar(_))
        ));
    }
}
