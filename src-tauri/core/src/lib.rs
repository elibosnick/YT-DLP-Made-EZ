//! Pure logic for Video Downloader.
//!
//! Everything here is free of Tauri, process spawning, and filesystem access, so it can
//! be exercised by fast unit tests. The Tauri crate in `../src` is a thin adapter over
//! this: it spawns yt-dlp, feeds the output through these parsers, and forwards the
//! results to the frontend.
//!
//! If you are fixing a bug in how a URL is accepted, how yt-dlp is invoked, how its
//! progress output is read, or what error text the user sees — it is in this crate, and
//! it has a test.

pub mod args;
pub mod error;
pub mod progress;
pub mod urls;

pub use args::{build_ytdlp_args, Format};
pub use error::{interpret_ytdlp_error, AppError, FriendlyError, Result};
pub use progress::{
    is_conversion_line, parse_filepath_line, parse_progress_line, DownloadProgress,
    FILEPATH_MARKER, PROGRESS_MARKER,
};
pub use urls::validate_url;
