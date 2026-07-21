//! Thin wrapper around `tokio::process::Command`.
//!
//! Exists for one reason: on Windows, spawning a console executable from a GUI app pops
//! a black terminal window on screen for the life of the process. For our audience that
//! looks like the computer has been hacked. `CREATE_NO_WINDOW` suppresses it.
//!
//! Every spawn of yt-dlp or ffmpeg must go through here.

use std::path::Path;

use tokio::process::Command;

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

pub fn command(program: &Path) -> Command {
    let mut cmd = Command::new(program);

    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }

    // Kill the child if the handle is dropped, so a cancelled or panicking download
    // cannot leave an orphaned yt-dlp running in the background.
    cmd.kill_on_drop(true);
    cmd
}
