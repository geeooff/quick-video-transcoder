//! Updating the application from its GitHub releases, from a window the user opens (*Help → Check for
//! updates*): check, download and verify, then swap the executable and restart.
//!
//! Rules this module is shaped by (docs/updating.md has the design and its measurements):
//! - **Never a silent connection.** Nothing connects unless the user clicked; no check at start.
//! - **The UI reflects [`Machine`]**: every rule (which actions a phase takes, what a result means) lives
//!   here; the window draws [`Machine::phase`] and sends [`Event`]s. The worker runs the [`Effect`]s.
//! - **One release, named by its tag.** The check reads the latest release once; the download uses that
//!   release's own URLs, so a release published in between cannot mix one version's file with another's
//!   hash. A tag that is not exactly `vX.Y.Z` is "a release this version does not understand". Never a
//!   downgrade.
//! - **Two witnesses to the file**: the workflow's `SHA256SUMS.txt` and GitHub's own digest of the asset,
//!   both fetched over TLS, must equal the downloaded file's SHA-256. They protect against a corrupted or
//!   truncated download, not against a compromised account (said in the docs).
//! - **The running executable is never written.** The new one is downloaded beside it
//!   (`<exe>.new`, same folder, so the swap is a rename), and a hidden PowerShell started at exit waits for
//!   this process to end, renames, restarts, and puts the old one back on failure ([`install`]). The
//!   version that starts next says whether it worked, from `pending.txt` ([`install::settle`]).
//! - Tests never call GitHub: [`feed::Feed`] is scripted, and the HTTP feed is tested against a listener on
//!   127.0.0.1.

pub mod feed;
pub mod install;

use std::fmt;

/// A version, `major.minor.patch`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Version(pub u16, pub u16, pub u16);

impl Version {
    /// This executable's version.
    pub fn current() -> Self {
        Self::parse(crate::build_info::VERSION).expect("Cargo.toml has an x.y.z version")
    }

    /// `1.2.3`, without suffix.
    pub fn parse(text: &str) -> Option<Self> {
        let mut parts = text.split('.').map(|p| {
            (!p.is_empty() && p.bytes().all(|b| b.is_ascii_digit()))
                .then(|| p.parse::<u16>().ok())
                .flatten()
        });
        let version = Self(parts.next()??, parts.next()??, parts.next()??);
        parts.next().is_none().then_some(version)
    }

    /// A release tag, exactly `vX.Y.Z`.
    pub fn from_tag(tag: &str) -> Option<Self> {
        Self::parse(tag.strip_prefix('v')?)
    }
}

impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}", self.0, self.1, self.2)
    }
}

/// What the latest release offers.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Release {
    /// Its version, from the tag.
    pub version: Version,
    /// Its page on GitHub.
    pub page: String,
    /// Its notes (the changelog section), Markdown.
    pub notes: String,
    /// The executable's download URL.
    pub exe_url: String,
    /// The executable's size in bytes.
    pub exe_size: u64,
    /// GitHub's SHA-256 of the executable, lower-case hex, when GitHub gives it.
    pub exe_digest: Option<String>,
    /// `SHA256SUMS.txt`'s download URL.
    pub sums_url: String,
}

/// Why a step failed. Shown to the user by the window (localized there); the log has the details.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Fault {
    /// No answer: no connection, DNS, timeout, TLS. The detail is technical.
    NoConnection(String),
    /// GitHub answered with this HTTP status.
    Status(u16),
    /// An answer this version does not understand (no such asset, a tag not `vX.Y.Z`, not JSON…).
    Unexpected(String),
    /// The downloaded file's SHA-256 is not the published one.
    NotVerified,
    /// The executable's folder cannot be written (installed under Program Files, read-only…).
    CannotWrite(String),
    /// The restart could not be started.
    Launch(String),
}

/// The updater's state. Starts [`Phase::Idle`], or with the outcome of an update that ran before this
/// start ([`install::settle`]).
#[derive(Clone, Debug, PartialEq)]
pub enum Phase {
    /// Nothing asked.
    Idle,
    /// The check is running.
    Checking,
    /// This version is the latest.
    UpToDate(Version),
    /// A newer release exists.
    Available(Release),
    /// Downloading it: bytes so far.
    Downloading(Release, u64),
    /// Downloaded and verified beside the executable: restarting installs it.
    Ready(Release),
    /// The swap script is starting: the application closes now.
    Restarting(Release),
    /// The previous run updated to this version (said once, at start).
    Updated(Version),
    /// A step failed; `release` is kept when the failure came after a release was found.
    Failed {
        /// What failed.
        fault: Fault,
        /// The release found, if any (to offer its page).
        release: Option<Release>,
    },
}

/// What happened, sent by the window (a click) or the worker (a result).
#[derive(Clone, Debug, PartialEq)]
pub enum Event {
    /// *Check for updates*.
    CheckAsked,
    /// The check's result: the latest release, or why it failed.
    Checked(Result<Release, Fault>),
    /// *Download and install*.
    DownloadAsked,
    /// Bytes downloaded so far.
    Progress(u64),
    /// The download's result (verified or not).
    Downloaded(Result<(), Fault>),
    /// *Restart now* (the window checks nothing is encoding first).
    RestartAsked,
    /// The swap script could not be started.
    LaunchFailed(Fault),
}

/// Work the worker must do for an event.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Effect {
    /// Read the latest release.
    Check,
    /// Download and verify this release beside the executable.
    Download(Release),
    /// Write `pending.txt`, start the swap script and close the application.
    Restart(Release),
}

/// The update state machine. Pure: no I/O, so every rule is tested.
#[derive(Clone, Debug, PartialEq)]
pub struct Machine {
    phase: Phase,
    current: Version,
}

impl Machine {
    /// A machine for the running version, starting in `phase`.
    pub fn new(current: Version, phase: Phase) -> Self {
        Self { phase, current }
    }

    /// The current phase, for the window.
    pub fn phase(&self) -> &Phase {
        &self.phase
    }

    /// Applies an event and returns the work it asks for. An event the phase does not take (a click on a
    /// stale button, a result nobody waits for) changes nothing.
    pub fn apply(&mut self, event: Event) -> Option<Effect> {
        use Phase as P;
        let (phase, effect) = match (&self.phase, event) {
            (
                P::Idle | P::UpToDate(_) | P::Available(_) | P::Updated(_) | P::Failed { .. },
                Event::CheckAsked,
            ) => (P::Checking, Some(Effect::Check)),
            (P::Checking, Event::Checked(Ok(release))) => {
                if release.version > self.current {
                    (P::Available(release), None)
                } else {
                    // Equal, or older (a release withdrawn): never offered as an update.
                    (P::UpToDate(self.current), None)
                }
            }
            (P::Checking, Event::Checked(Err(fault))) => (
                P::Failed {
                    fault,
                    release: None,
                },
                None,
            ),
            (P::Available(release), Event::DownloadAsked)
            | (
                P::Failed {
                    release: Some(release),
                    ..
                },
                Event::DownloadAsked,
            ) => (
                P::Downloading(release.clone(), 0),
                Some(Effect::Download(release.clone())),
            ),
            (P::Downloading(release, _), Event::Progress(done)) => {
                (P::Downloading(release.clone(), done), None)
            }
            (P::Downloading(release, _), Event::Downloaded(Ok(()))) => {
                (P::Ready(release.clone()), None)
            }
            (P::Downloading(release, _), Event::Downloaded(Err(fault))) => (
                P::Failed {
                    fault,
                    release: Some(release.clone()),
                },
                None,
            ),
            (P::Ready(release), Event::RestartAsked) => (
                P::Restarting(release.clone()),
                Some(Effect::Restart(release.clone())),
            ),
            (P::Restarting(release), Event::LaunchFailed(fault)) => (
                P::Failed {
                    fault,
                    release: Some(release.clone()),
                },
                None,
            ),
            _ => return None,
        };
        self.phase = phase;
        effect
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn release(version: Version) -> Release {
        Release {
            version,
            page: format!("https://example.invalid/v{version}"),
            notes: "### Added\n\n- something".to_owned(),
            exe_url: "https://example.invalid/exe".to_owned(),
            exe_size: 1000,
            exe_digest: Some("ab".repeat(32)),
            sums_url: "https://example.invalid/sums".to_owned(),
        }
    }

    #[test]
    fn reads_versions_and_tags_strictly() {
        assert_eq!(Version::parse("1.10.0"), Some(Version(1, 10, 0)));
        assert_eq!(Version::from_tag("v2.0.1"), Some(Version(2, 0, 1)));
        for bad in [
            "1.0",
            "1.0.0.0",
            "1.0.0-beta",
            "v1.0.0",
            "1..0",
            "",
            "a.b.c",
        ] {
            assert_eq!(Version::parse(bad), None, "{bad}");
        }
        assert_eq!(Version::from_tag("1.0.0"), None);
        assert_eq!(Version::from_tag("v1.0.0-rc1"), None);
        assert!(Version(1, 10, 0) > Version(1, 9, 9));
        assert_eq!(Version(1, 2, 3).to_string(), "1.2.3");
    }

    #[test]
    fn checks_downloads_and_restarts_in_order() {
        let mut machine = Machine::new(Version(1, 0, 0), Phase::Idle);
        assert_eq!(
            machine.apply(Event::DownloadAsked),
            None,
            "nothing to download yet"
        );
        assert_eq!(machine.apply(Event::CheckAsked), Some(Effect::Check));
        assert_eq!(
            machine.apply(Event::CheckAsked),
            None,
            "one check at a time"
        );
        let newer = release(Version(1, 1, 0));
        assert_eq!(machine.apply(Event::Checked(Ok(newer.clone()))), None);
        assert_eq!(machine.phase(), &Phase::Available(newer.clone()));
        assert_eq!(
            machine.apply(Event::DownloadAsked),
            Some(Effect::Download(newer.clone()))
        );
        assert_eq!(
            machine.apply(Event::CheckAsked),
            None,
            "not during a download"
        );
        machine.apply(Event::Progress(500));
        assert_eq!(machine.phase(), &Phase::Downloading(newer.clone(), 500));
        machine.apply(Event::Downloaded(Ok(())));
        assert_eq!(machine.phase(), &Phase::Ready(newer.clone()));
        assert_eq!(
            machine.apply(Event::RestartAsked),
            Some(Effect::Restart(newer.clone()))
        );
        machine.apply(Event::LaunchFailed(Fault::Launch("x".to_owned())));
        assert!(matches!(machine.phase(), Phase::Failed { release: Some(r), .. } if *r == newer));
    }

    #[test]
    fn never_offers_the_same_or_an_older_version() {
        for latest in [Version(1, 0, 0), Version(0, 9, 0)] {
            let mut machine = Machine::new(Version(1, 0, 0), Phase::Idle);
            machine.apply(Event::CheckAsked);
            machine.apply(Event::Checked(Ok(release(latest))));
            assert_eq!(machine.phase(), &Phase::UpToDate(Version(1, 0, 0)));
            assert_eq!(machine.apply(Event::DownloadAsked), None);
        }
    }

    #[test]
    fn a_failed_download_can_be_retried_and_a_failed_check_checked_again() {
        let newer = release(Version(1, 1, 0));
        let mut machine = Machine::new(Version(1, 0, 0), Phase::Available(newer.clone()));
        machine.apply(Event::DownloadAsked);
        machine.apply(Event::Downloaded(Err(Fault::NotVerified)));
        assert_eq!(
            machine.apply(Event::DownloadAsked),
            Some(Effect::Download(newer))
        );
        let mut offline = Machine::new(Version(1, 0, 0), Phase::Checking);
        offline.apply(Event::Checked(Err(Fault::NoConnection("dns".to_owned()))));
        assert_eq!(offline.apply(Event::DownloadAsked), None);
        assert_eq!(offline.apply(Event::CheckAsked), Some(Effect::Check));
    }
}
