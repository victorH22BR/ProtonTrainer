//! Steam discovery: installations, libraries and installed games.
//!
//! Planned responsibilities (see `docs/ARCHITECTURE.md`):
//! - locate Steam roots (native, Flatpak, user-provided) without relying on a
//!   single fixed path;
//! - read `libraryfolders.vdf` and `appmanifest_*.acf`;
//! - expose installed games as `name`, `AppId` and install path.
//!
//! Status: skeleton (Phase 0). The implementation arrives in Phase 1.
