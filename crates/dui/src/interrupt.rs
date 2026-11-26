//! Cleanup when a run is interrupted.
//!
//! Staging directories, browser profiles and child processes are released by `Drop`, which
//! does not run when the process is killed. Each one registers here while it lives. The first
//! registration starts a thread that waits for SIGINT, SIGTERM or SIGHUP, kills the registered
//! processes, removes the registered directories and exits with 128 + the signal number, as
//! the default action would.

use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::{Mutex, Once},
};

#[derive(Clone, PartialEq)]
enum Resource {
    Dir(PathBuf),
    Process(u32),
}

static REGISTERED: Mutex<Vec<Resource>> = Mutex::new(Vec::new());
static WATCH: Once = Once::new();
/// The signal being handled, or 0.
static SIGNAL: std::sync::atomic::AtomicI32 = std::sync::atomic::AtomicI32::new(0);

/// After an interrupt, the main thread must not finish first: work fails once the browsers are
/// killed, and returning would exit with that error before cleanup is done. It waits here for
/// the cleanup thread to exit the process with 128 + the signal number.
pub(crate) fn wait_if_interrupted() {
    while SIGNAL.load(std::sync::atomic::Ordering::SeqCst) != 0 {
        std::thread::park_timeout(std::time::Duration::from_millis(100));
    }
}

/// Unregisters its resource when dropped, after the owner released it the normal way.
pub(crate) struct Guard(Resource);

impl Drop for Guard {
    fn drop(&mut self) {
        if let Ok(mut registered) = REGISTERED.lock()
            && let Some(at) = registered.iter().position(|r| *r == self.0)
        {
            registered.remove(at);
        }
    }
}

fn register(resource: Resource) -> Guard {
    watch();
    if let Ok(mut registered) = REGISTERED.lock() {
        registered.push(resource.clone());
    }
    Guard(resource)
}

/// Remove `path` if dui is interrupted before the guard is dropped.
pub(crate) fn remove_on_interrupt(path: &Path) -> Guard {
    register(Resource::Dir(path.to_path_buf()))
}

/// Kill process `pid` if dui is interrupted before the guard is dropped.
pub(crate) fn kill_on_interrupt(pid: u32) -> Guard {
    register(Resource::Process(pid))
}

fn watch() {
    use signal_hook::consts::{SIGHUP, SIGINT, SIGTERM};
    WATCH.call_once(|| {
        let Ok(mut signals) = signal_hook::iterator::Signals::new([SIGINT, SIGTERM, SIGHUP]) else {
            return;
        };
        std::thread::spawn(move || {
            if let Some(signal) = signals.forever().next() {
                SIGNAL.store(signal, std::sync::atomic::Ordering::SeqCst);
                let registered = REGISTERED.lock().map(|r| r.clone()).unwrap_or_default();
                // Processes first, so nothing is still writing into a directory being removed.
                for resource in &registered {
                    if let Resource::Process(pid) = resource {
                        let _ = Command::new("kill")
                            .args(["-KILL", &pid.to_string()])
                            .stderr(Stdio::null())
                            .status();
                    }
                }
                for resource in &registered {
                    if let Resource::Dir(dir) = resource {
                        let _ = fs::remove_dir_all(dir);
                    }
                }
                std::process::exit(128 + signal);
            }
        });
    });
}
