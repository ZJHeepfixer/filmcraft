//! Installed faces the interface falls back to: Hangul in every build, and Chinese, Japanese,
//! Arabic and Hebrew when the craft-fonts don't cover them (see [`crate::cjk`], [`crate::rtl`]
//! and [`crate::i18n`]).
//!
//! A face's file is read once, into the font database's face, which keeps it for the whole
//! process, and egui is given those same bytes ([`Face::shared_bytes`]) as one more reference to
//! them. egui 0.37's `FontData` holds its file as a shared blob, so egui no longer copies it, but
//! handing it a copy of the file ([`Face::data`]) still meant two copies of every face: the
//! database's and ours (Apple SD Gothic Neo, 53 MB, twice).

use std::sync::{Arc, OnceLock};

use egui::FontData;
use filmcraft_text::fonts::Face;

/// An installed face, found by characters it must cover.
pub(crate) struct SystemFace {
    /// Characters the face must cover.
    sample: &'static str,
    /// Installed families to prefer, best first.
    preferred: &'static [&'static str],
    found: OnceLock<Option<Arc<Face>>>,
}

impl SystemFace {
    pub(crate) const fn new(sample: &'static str, preferred: &'static [&'static str]) -> Self {
        Self { sample, preferred, found: OnceLock::new() }
    }

    /// The installed face (see [`find`]), looked up once per process. `None` on the web and on
    /// systems without one.
    pub(crate) fn face(&self) -> Option<&Arc<Face>> {
        self.found.get_or_init(|| find(self.sample, self.preferred)).as_ref()
    }

    /// The face for egui, sharing the font database's bytes (`None` without a face, or when its
    /// file can't be read).
    pub(crate) fn font_data(&self) -> Option<Arc<FontData>> {
        shared(self.face()?)
    }
}

/// `face` for egui, sharing the bytes the font database keeps for it: no copy, however often the
/// fonts are installed. `None` when the face's file can't be read.
pub(crate) fn shared(face: &Face) -> Option<Arc<FontData>> {
    Some(Arc::new(FontData::from_blob(face.shared_bytes()?, face.info.index)))
}

/// The installed face that covers `sample`: the first family of `preferred` that has one (its
/// style closest to regular), else any installed face that does. A font collection's face index
/// matters: Windows CJK UI fonts commonly live in TTC files. Coverage is probed without keeping
/// the files ([`Face::covers_text`]). Scans the system font folders on first use; `None` on the
/// web.
pub(crate) fn find(sample: &str, preferred: &[&str]) -> Option<Arc<Face>> {
    if cfg!(target_arch = "wasm32") {
        return None;
    }
    filmcraft_text::fonts::scan_system();
    let faces: Vec<_> = filmcraft_text::fonts::all_faces().into_iter().filter(|f| f.info.origin == "system" && !f.info.italic).collect();
    let covers = |f: &Face| f.covers_text(sample);
    let by_weight = |f: &&Arc<Face>| f.info.weight.abs_diff(400);
    let found = preferred.iter().find_map(|name| faces.iter().filter(|f| f.info.family.eq_ignore_ascii_case(name) && covers(f)).min_by_key(by_weight));
    found.or_else(|| faces.iter().filter(|f| covers(f)).min_by_key(by_weight)).cloned()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The installed fallbacks reach egui without a copy: every install hands egui the bytes the
    /// font database keeps, at the same address.
    #[test]
    fn installed_faces_are_shared_not_copied() {
        static INSTALLED: SystemFace = SystemFace::new("A", &["Arial", "Helvetica", "DejaVu Sans", "Segoe UI"]);
        let Some(face) = INSTALLED.face() else {
            eprintln!("SKIPPED: no installed Latin font");
            return;
        };
        let (Some(a), Some(b)) = (INSTALLED.font_data(), INSTALLED.font_data()) else {
            panic!("the installed face's file could not be read");
        };
        let Some(kept) = face.shared_bytes() else { panic!("the database keeps no bytes for the face") };
        for data in [&a, &b] {
            assert_eq!(data.bytes().as_ptr(), (*kept).as_ref().as_ptr(), "egui must see the database's own bytes, not a copy");
            assert_eq!(data.index, face.info.index);
        }
    }
}
