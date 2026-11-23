//! One window at a time: a second launch (another *Send to*, a double-click) hands its files to the
//! window already open and exits, so two queues never compete for the same hardware encoder.
//!
//! The first instance listens on a loopback TCP port chosen by the system and writes the port with a
//! random token to `instance.txt` in the data folder. A later launch reads it, connects, sends the token
//! and its files one per line, and exits once the first instance acknowledges. Anything unexpected (no
//! file, a stale port, a wrong token, a path that is not UTF-8) makes the launch open its own window: a
//! second window is the worst outcome, never lost files.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{Ipv4Addr, SocketAddr, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use crossbeam_channel::{Receiver, Sender, unbounded};

/// The file naming the port and token of the running instance.
pub const FILE_NAME: &str = "instance.txt";

/// A callback waking the window when files arrive.
type Waker = Arc<Mutex<Option<Box<dyn Fn() + Send>>>>;

/// How a launch goes on.
pub enum Launch {
    /// This is the first instance: open the window and receive later launches' files.
    Primary(Incoming),
    /// The files went to the running instance: exit.
    Forwarded,
}

/// Files sent by later launches.
pub struct Incoming {
    receiver: Receiver<Vec<PathBuf>>,
    waker: Waker,
    /// Why later launches cannot reach this one, to log once the log is open.
    problem: Option<String>,
}

impl Incoming {
    /// Why later launches will open windows of their own, if they will.
    pub fn problem(&self) -> Option<&str> {
        self.problem.as_deref()
    }

    /// Calls `wake` whenever files arrive (to repaint the window, which then calls [`Incoming::take`]).
    pub fn set_waker(&self, wake: impl Fn() + Send + 'static) {
        if let Ok(mut waker) = self.waker.lock() {
            *waker = Some(Box::new(wake));
        }
    }

    /// Files received since the last call, in arrival order.
    pub fn take(&self) -> Vec<PathBuf> {
        self.receiver.try_iter().flatten().collect()
    }
}

/// Hands `files` to the running instance whose details are in `dir`, or becomes that instance.
pub fn claim(dir: &Path, files: &[PathBuf]) -> Launch {
    if forward(dir, files).is_some() {
        return Launch::Forwarded;
    }
    match listen(dir) {
        Ok(incoming) => Launch::Primary(incoming),
        Err(error) => Launch::Primary(Incoming {
            receiver: unbounded().1,
            waker: Arc::default(),
            problem: Some(format!("{error:#}")),
        }),
    }
}

/// Sends the files to a running instance; `Some` once it acknowledged them.
fn forward(dir: &Path, files: &[PathBuf]) -> Option<()> {
    let text = std::fs::read_to_string(dir.join(FILE_NAME)).ok()?;
    let (port, token) = text.trim().split_once(' ')?;
    let address = SocketAddr::from((Ipv4Addr::LOCALHOST, port.parse::<u16>().ok()?));
    let mut stream = TcpStream::connect_timeout(&address, Duration::from_millis(500)).ok()?;
    stream.set_read_timeout(Some(Duration::from_secs(3))).ok()?;
    stream
        .set_write_timeout(Some(Duration::from_secs(3)))
        .ok()?;
    let mut message = format!("{token}\n");
    for file in files {
        // A line per path: a path holding a line break or invalid UTF-8 opens its own window instead.
        let path = file.to_str().filter(|p| !p.contains(['\n', '\r']))?;
        message.push_str(path);
        message.push('\n');
    }
    message.push('\n');
    stream.write_all(message.as_bytes()).ok()?;
    let mut reply = String::new();
    BufReader::new(stream).read_line(&mut reply).ok()?;
    (reply.trim() == "ok").then_some(())
}

/// Listens for later launches and publishes the port and a fresh token in `dir`.
fn listen(dir: &Path) -> anyhow::Result<Incoming> {
    use anyhow::Context;
    let listener =
        TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).context("cannot listen on loopback")?;
    let port = listener.local_addr()?.port();
    let token = token();
    std::fs::create_dir_all(dir).with_context(|| format!("cannot create {}", dir.display()))?;
    // Written whole then renamed, so a launch never reads half a file.
    let path = dir.join(FILE_NAME);
    let temporary = dir.join(format!("{FILE_NAME}.tmp"));
    std::fs::write(&temporary, format!("{port} {token}\n"))
        .with_context(|| format!("cannot write {}", temporary.display()))?;
    std::fs::rename(&temporary, &path)
        .with_context(|| format!("cannot write {}", path.display()))?;
    let (sender, receiver) = unbounded();
    let waker: Waker = Arc::default();
    let wake = Arc::clone(&waker);
    std::thread::Builder::new()
        .name("single instance".to_owned())
        .spawn(move || {
            for stream in listener.incoming().flatten() {
                serve(stream, &token, &sender, &wake);
            }
        })?;
    Ok(Incoming {
        receiver,
        waker,
        problem: None,
    })
}

/// Reads one launch's files; anything malformed is dropped without an answer.
fn serve(stream: TcpStream, token: &str, sender: &Sender<Vec<PathBuf>>, waker: &Waker) {
    let _ = stream.set_read_timeout(Some(Duration::from_secs(3)));
    let Ok(mut reply) = stream.try_clone() else {
        return;
    };
    // A launch sends a few paths: a megabyte is far more than any real message.
    let mut lines = BufReader::new(stream.take(1 << 20)).lines();
    if lines.next().and_then(Result::ok).as_deref() != Some(token) {
        return;
    }
    let mut files = Vec::new();
    for line in lines {
        match line {
            Ok(line) if line.is_empty() => break,
            Ok(line) => files.push(PathBuf::from(line)),
            Err(_) => return,
        }
    }
    if sender.send(files).is_ok() {
        let _ = reply.write_all(b"ok\n");
        if let Ok(waker) = waker.lock()
            && let Some(wake) = waker.as_ref()
        {
            wake();
        }
    }
}

/// A token no other program can guess: two hashes seeded by the system's per-process random keys.
fn token() -> String {
    use std::hash::{BuildHasher, RandomState};
    let seed = (std::process::id(), std::time::SystemTime::now());
    let first = RandomState::new().hash_one(seed);
    let second = RandomState::new().hash_one((seed, first));
    format!("{first:016x}{second:016x}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_second_launch_hands_its_files_to_the_first() {
        let dir = tempfile::tempdir().unwrap();
        let Launch::Primary(incoming) = claim(dir.path(), &[]) else {
            panic!("the first launch must open the window");
        };
        let woken = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let count = Arc::clone(&woken);
        incoming.set_waker(move || {
            count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        });
        let files = vec![
            PathBuf::from(r"C:\Vidéos\clip 1.mp4"),
            PathBuf::from("b.mkv"),
        ];
        assert!(matches!(claim(dir.path(), &files), Launch::Forwarded));
        assert_eq!(incoming.take(), files);
        assert_eq!(woken.load(std::sync::atomic::Ordering::SeqCst), 1);
    }

    #[test]
    fn a_wrong_token_or_a_stale_file_opens_a_window() {
        let dir = tempfile::tempdir().unwrap();
        let Launch::Primary(incoming) = claim(dir.path(), &[]) else {
            panic!("the first launch must open the window");
        };
        let path = dir.path().join(FILE_NAME);
        let text = std::fs::read_to_string(&path).unwrap();
        let port = text.split_once(' ').unwrap().0.to_owned();
        std::fs::write(&path, format!("{port} wrong\n")).unwrap();
        assert!(forward(dir.path(), &[PathBuf::from("a.mp4")]).is_none());
        assert!(incoming.take().is_empty());
        std::fs::write(&path, "1 stale\n").unwrap();
        assert!(forward(dir.path(), &[PathBuf::from("a.mp4")]).is_none());
    }
}
