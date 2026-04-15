use crate::types::BookMetadata;
use crate::core::GutenCore;
use chrono::{SecondsFormat, Utc};

impl GutenCore {
    pub fn get_metadata(&self) -> Option<&BookMetadata> {
        self.metadata.as_ref()
    }

    pub fn set_metadata(&mut self, title: Option<String>, language: Option<String>, identifier: Option<String>) {
        if let Some(ref mut md) = self.metadata {
            let mut changed = false;
            if let Some(t) = title {
                md.title = t;
                changed = true;
            }
            if let Some(l) = language {
                md.language = l;
                changed = true;
            }
            if let Some(i) = identifier {
                md.identifier = i;
                changed = true;
            }

            if changed {
                md.modified = Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true);
            }
        }
    }

    pub(crate) fn update_modified_date(&mut self) {
        if let Some(ref mut md) = self.metadata {
            md.modified = Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true);
        }
    }
}
