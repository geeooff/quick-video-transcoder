//! The queue of videos and the background work on them: probing, size estimates and encodes.
//!
//! Each video goes through: probe (ffprobe, then whether Vulkan decodes it) → estimate, redone in the
//! background whenever its settings or trim change and stay still for a moment → encode, once the user
//! asks. Workers are plain threads reporting through one channel; the UI applies the events in
//! [`Queue::poll`] and never waits on a worker.
//!
//! Encoder sessions are shared through [`Slots`]: the estimate's samples and the encodes each hold one
//! while FFmpeg runs. Two encodes at a time is the default: on an RTX 4090, 4K60 AV1 HDR to 720p30 runs
//! at 5.2x real time alone and 7x in total with two, no faster with four or six.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex, PoisonError};
use std::time::{Duration, Instant};

use crossbeam_channel::{Receiver, Sender, unbounded};

use crate::encode::{self, Encoded, Progress};
use crate::estimate::{self, Estimate, EstimateRequest};
use crate::ffmpeg::encoders::EncoderBackend;
use crate::ffmpeg::filters::Engine;
use crate::ffmpeg::hardware::{self, Capabilities};
use crate::ffmpeg::probe::{MediaInfo, probe};
use crate::naming;
use crate::profile::Profile;

/// A counting semaphore: at most `capacity` holders at a time.
pub struct Slots {
    state: Mutex<(usize, usize)>,
    freed: Condvar,
}

/// One held slot, given back when dropped.
pub struct Slot<'a>(&'a Slots);

impl Drop for Slot<'_> {
    fn drop(&mut self) {
        let mut state = self.0.state.lock().unwrap_or_else(PoisonError::into_inner);
        state.0 -= 1;
        self.0.freed.notify_all();
    }
}

impl Slots {
    /// `capacity` slots (at least one).
    pub fn new(capacity: usize) -> Self {
        Self {
            state: Mutex::new((0, capacity.max(1))),
            freed: Condvar::new(),
        }
    }

    /// Changes the capacity; holders above it finish normally.
    pub fn set_capacity(&self, capacity: usize) {
        self.state.lock().unwrap_or_else(PoisonError::into_inner).1 = capacity.max(1);
        self.freed.notify_all();
    }

    /// Waits for a free slot.
    pub fn acquire(&self) -> Slot<'_> {
        let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        while state.0 >= state.1 {
            state = self
                .freed
                .wait(state)
                .unwrap_or_else(PoisonError::into_inner);
        }
        state.0 += 1;
        Slot(self)
    }
}

/// Identifies a video in the queue.
pub type JobId = u64;

/// The kept part of a video, in source seconds.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Trim {
    /// Start of the kept part.
    pub start: f64,
    /// End of the kept part.
    pub end: f64,
}

impl Trim {
    /// Duration of the kept part.
    pub fn duration(self) -> f64 {
        (self.end - self.start).max(0.0)
    }
}

/// Where a video stands.
#[derive(Clone, Debug, PartialEq)]
pub enum Phase {
    /// Reading its media information.
    Probing,
    /// It cannot be used (not a video, unreadable).
    Unusable(String),
    /// Waiting: an estimate may run, an encode may start.
    Idle,
    /// Its estimate runs.
    Estimating,
    /// Waiting for an encoder slot.
    Waiting,
    /// Encoding.
    Encoding(Progress),
    /// Encoded.
    Done(Encoded),
    /// The encode failed.
    Failed(String),
}

/// One video of the queue.
pub struct Job {
    /// Its identifier.
    pub id: JobId,
    /// The source file.
    pub path: PathBuf,
    /// Its media information, once probed.
    pub media: Option<MediaInfo>,
    /// How its filters run, once probed (and corrected if the estimate had to fall back).
    pub engine: Option<Engine>,
    /// Its own copy of a profile.
    pub profile: Profile,
    /// The profile it was copied from.
    pub profile_name: String,
    /// The kept part.
    pub trim: Trim,
    /// The last estimate and the settings it was made with.
    pub estimate: Option<(EstimateKey, Result<Estimate, String>)>,
    /// Where it stands.
    pub phase: Phase,
    /// The user asked to encode it.
    pub wanted: bool,
    /// When its settings or trim last changed (the estimate waits for them to settle).
    pub changed: Instant,
    cancel: Arc<AtomicBool>,
}

/// What an estimate depends on.
#[derive(Clone, Debug, PartialEq)]
pub struct EstimateKey {
    profile: Profile,
    start: f64,
    end: f64,
    encoder: &'static str,
}

impl Job {
    /// The file name, for the UI.
    pub fn name(&self) -> String {
        self.path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default()
    }

    /// The estimate, when it matches the current settings.
    pub fn current_estimate(&self, key: Option<&EstimateKey>) -> Option<&Result<Estimate, String>> {
        let (made_with, result) = self.estimate.as_ref()?;
        (Some(made_with) == key).then_some(result)
    }

    /// Records a change of settings or trim: the estimate is redone once they settle.
    pub fn touch(&mut self) {
        self.changed = Instant::now();
        if let Phase::Done(_) | Phase::Failed(_) = self.phase {
            self.phase = Phase::Idle;
        }
    }

    /// `true` while a worker runs for it.
    pub fn busy(&self) -> bool {
        matches!(
            self.phase,
            Phase::Probing | Phase::Estimating | Phase::Waiting | Phase::Encoding(_)
        )
    }
}

/// Messages from the workers.
enum Event {
    Probed(JobId, Result<(MediaInfo, Engine), String>),
    Estimated(JobId, EstimateKey, Engine, Result<Estimate, String>),
    Started(JobId),
    Progress(JobId, Progress),
    Encoded(JobId, Result<Encoded, String>),
}

/// Settings of the queue, from the preferences.
#[derive(Clone, Debug, PartialEq)]
pub struct QueueSettings {
    /// Encodes running at once.
    pub parallel_encodes: usize,
    /// Preferred encoder backend, `None` for the best that works.
    pub backend: Option<EncoderBackend>,
    /// Output folder, `None` for next to each source.
    pub output_dir: Option<PathBuf>,
}

impl Default for QueueSettings {
    fn default() -> Self {
        Self {
            parallel_encodes: 2,
            backend: None,
            output_dir: None,
        }
    }
}

/// The settings must stay still this long before an estimate starts.
const SETTLE: Duration = Duration::from_millis(700);
/// Estimates running at once (each runs its samples in parallel through the slots).
const PARALLEL_ESTIMATES: usize = 2;
/// Encoder sessions the estimate samples may hold at once.
const SAMPLE_SLOTS: usize = 4;

/// The queue.
pub struct Queue {
    /// The videos, in order.
    pub jobs: Vec<Job>,
    next_id: JobId,
    sender: Sender<Event>,
    receiver: Receiver<Event>,
    sample_slots: Arc<Slots>,
    encode_slots: Arc<Slots>,
    /// What works on this machine.
    pub capabilities: Capabilities,
    /// Queue settings.
    pub settings: QueueSettings,
    notify: Arc<dyn Fn() + Send + Sync>,
    paused: bool,
}

impl Queue {
    /// An empty queue; `notify` is called from worker threads when something changed (a repaint).
    pub fn new(
        capabilities: Capabilities,
        settings: QueueSettings,
        notify: Arc<dyn Fn() + Send + Sync>,
    ) -> Self {
        let (sender, receiver) = unbounded();
        Self {
            jobs: Vec::new(),
            next_id: 1,
            sender,
            receiver,
            sample_slots: Arc::new(Slots::new(SAMPLE_SLOTS)),
            encode_slots: Arc::new(Slots::new(settings.parallel_encodes)),
            capabilities,
            settings,
            notify,
            paused: false,
        }
    }

    /// Applies new settings.
    pub fn set_settings(&mut self, settings: QueueSettings) {
        self.encode_slots.set_capacity(settings.parallel_encodes);
        // Only the encoder changes what an estimate says: the output folder and the encodes at once do not.
        let encoder_changed = settings.backend != self.settings.backend;
        self.settings = settings;
        if encoder_changed {
            for job in &mut self.jobs {
                job.touch();
            }
        }
    }

    /// Applies the hardware detection's result.
    pub fn set_capabilities(&mut self, capabilities: Capabilities) {
        if !capabilities.placebo {
            for job in &mut self.jobs {
                job.engine = job.engine.map(|_| Engine::SOFTWARE);
            }
        }
        self.capabilities = capabilities;
        for job in &mut self.jobs {
            job.touch();
        }
    }

    /// Adds a video with a copy of `profile` and starts probing it. A file already queued is not added
    /// twice; its id is returned.
    pub fn add(&mut self, path: PathBuf, profile: &Profile) -> JobId {
        if let Some(job) = self.jobs.iter().find(|j| naming::same_file(&j.path, &path)) {
            return job.id;
        }
        let id = self.next_id;
        self.next_id += 1;
        self.jobs.push(Job {
            id,
            path: path.clone(),
            media: None,
            engine: None,
            profile: profile.clone(),
            profile_name: profile.name.clone(),
            trim: Trim {
                start: 0.0,
                end: 0.0,
            },
            estimate: None,
            phase: Phase::Probing,
            wanted: false,
            changed: Instant::now(),
            cancel: Arc::new(AtomicBool::new(false)),
        });
        let sender = self.sender.clone();
        let notify = Arc::clone(&self.notify);
        let placebo = self.capabilities.placebo;
        std::thread::spawn(move || {
            let result = probe(&path).map_err(|e| format!("{e:#}")).map(|media| {
                let engine = Engine {
                    placebo,
                    vulkan_decode: placebo && hardware::vulkan_decodes(&path),
                };
                (media, engine)
            });
            let _ = sender.send(Event::Probed(id, result));
            notify();
        });
        id
    }

    /// The video with this id.
    pub fn get(&self, id: JobId) -> Option<&Job> {
        self.jobs.iter().find(|j| j.id == id)
    }

    /// The video with this id, to change it.
    pub fn get_mut(&mut self, id: JobId) -> Option<&mut Job> {
        self.jobs.iter_mut().find(|j| j.id == id)
    }

    /// Removes a video, stopping its work.
    pub fn remove(&mut self, id: JobId) {
        if let Some(job) = self.get(id) {
            job.cancel.store(true, Ordering::Relaxed);
        }
        self.jobs.retain(|j| j.id != id);
    }

    /// Stops the encode (or pending encode) of a video.
    pub fn cancel(&mut self, id: JobId) {
        if let Some(job) = self.get_mut(id) {
            job.wanted = false;
            if matches!(job.phase, Phase::Waiting | Phase::Encoding(_)) {
                job.cancel.store(true, Ordering::Relaxed);
            }
        }
    }

    /// Asks to encode these videos (as soon as their estimate is ready).
    pub fn encode(&mut self, ids: &[JobId]) {
        for job in self.jobs.iter_mut().filter(|j| ids.contains(&j.id)) {
            if !matches!(job.phase, Phase::Unusable(_)) && !job.busy() {
                job.wanted = true;
                if let Phase::Done(_) | Phase::Failed(_) = job.phase {
                    job.phase = Phase::Idle;
                }
            }
        }
    }

    /// The estimate key of a video with its current settings, `None` when it cannot be estimated.
    pub fn key(&self, job: &Job) -> Option<EstimateKey> {
        job.media.as_ref()?;
        let encoder = self
            .capabilities
            .encoder(job.profile.codec, self.settings.backend)?;
        Some(EstimateKey {
            profile: job.profile.clone(),
            start: job.trim.start,
            end: job.trim.end,
            encoder: encoder.name,
        })
    }

    /// Applies the workers' events and starts the work that can start. Returns lines for the log.
    pub fn poll(&mut self) -> Vec<(bool, String)> {
        let mut log = Vec::new();
        while let Ok(event) = self.receiver.try_recv() {
            self.apply(event, &mut log);
        }
        if !self.paused {
            self.start_estimates();
            self.start_encodes(&mut log);
        }
        log
    }

    /// Holds back new estimates and encodes (the work already running goes on), while the encoder's
    /// parallelism is measured: they would share the encoder with the measurement and skew it.
    pub fn set_paused(&mut self, paused: bool) {
        self.paused = paused;
    }

    fn apply(&mut self, event: Event, log: &mut Vec<(bool, String)>) {
        match event {
            Event::Probed(id, result) => {
                let Some(job) = self.get_mut(id) else { return };
                match result {
                    Ok((media, engine)) => {
                        log.push((
                            false,
                            format!(
                                "{}: {}, {}, {} audio track(s), color {}; {}",
                                job.name(),
                                media.summary(),
                                crate::timecode::format_short(media.duration, true),
                                media.audio.len(),
                                media.color_tags,
                                engine.label()
                            ),
                        ));
                        job.trim = Trim {
                            start: 0.0,
                            end: media.duration,
                        };
                        job.media = Some(media);
                        job.engine = Some(engine);
                        job.phase = Phase::Idle;
                        job.changed = Instant::now() - SETTLE;
                    }
                    Err(error) => {
                        log.push((true, format!("{}: {error}", job.name())));
                        job.phase = Phase::Unusable(error);
                    }
                }
            }
            Event::Estimated(id, key, engine, result) => {
                let Some(job) = self.get_mut(id) else { return };
                if job.phase == Phase::Estimating {
                    job.phase = Phase::Idle;
                }
                if let Err(error) = &result
                    && error != "cancelled"
                {
                    log.push((true, format!("{}: estimate failed: {error}", job.name())));
                }
                if job.engine != Some(engine) {
                    log.push((
                        false,
                        format!("{}: now using {}", job.name(), engine.label()),
                    ));
                    job.engine = Some(engine);
                }
                job.estimate = Some((key, result));
            }
            Event::Started(id) => {
                if let Some(job) = self.get_mut(id) {
                    job.phase = Phase::Encoding(Progress {
                        fraction: 0.0,
                        fps: None,
                        speed: None,
                        attempt: 1,
                    });
                }
            }
            Event::Progress(id, progress) => {
                if let Some(job) = self.get_mut(id)
                    && let Phase::Encoding(_) = job.phase
                {
                    job.phase = Phase::Encoding(progress);
                }
            }
            Event::Encoded(id, result) => {
                let Some(job) = self.get_mut(id) else { return };
                job.wanted = false;
                job.cancel = Arc::new(AtomicBool::new(false));
                job.phase = match result {
                    Ok(encoded) => {
                        log.push((
                            false,
                            format!(
                                "{}: encoded to {} ({:.2} MB in {:.1} s)",
                                job.name(),
                                encoded.output.display(),
                                encoded.bytes as f64 / 1e6,
                                encoded.elapsed.as_secs_f64()
                            ),
                        ));
                        Phase::Done(encoded)
                    }
                    Err(error) if error == "cancelled" => Phase::Idle,
                    Err(error) => {
                        log.push((true, format!("{}: encode failed: {error}", job.name())));
                        Phase::Failed(error)
                    }
                };
            }
        }
    }

    fn request(&self, job: &Job) -> Option<EstimateRequest> {
        let media = job.media.clone()?;
        Some(EstimateRequest {
            path: job.path.clone(),
            media,
            profile: job.profile.clone(),
            start: job.trim.start,
            end: job.trim.end,
            engine: job.engine?,
            encoder: self
                .capabilities
                .encoder(job.profile.codec, self.settings.backend)?,
        })
    }

    fn start_estimates(&mut self) {
        let running = self
            .jobs
            .iter()
            .filter(|j| j.phase == Phase::Estimating)
            .count();
        let mut free = PARALLEL_ESTIMATES.saturating_sub(running);
        // Videos waiting to be encoded first, then the rest in queue order.
        let mut order: Vec<usize> = (0..self.jobs.len()).collect();
        order.sort_by_key(|&i| !self.jobs[i].wanted);
        for index in order {
            if free == 0 {
                break;
            }
            let job = &self.jobs[index];
            if job.phase != Phase::Idle || job.changed.elapsed() < SETTLE {
                continue;
            }
            let Some(key) = self.key(job) else { continue };
            if job.current_estimate(Some(&key)).is_some() {
                continue;
            }
            let Some(request) = self.request(job) else {
                continue;
            };
            free -= 1;
            let (id, cancel) = (job.id, Arc::clone(&job.cancel));
            self.jobs[index].phase = Phase::Estimating;
            let sender = self.sender.clone();
            let notify = Arc::clone(&self.notify);
            let slots = Arc::clone(&self.sample_slots);
            let placebo = self.capabilities.placebo;
            std::thread::spawn(move || {
                let (engine, result) = estimate_with_fallback(request, placebo, &slots, &cancel);
                let _ = sender.send(Event::Estimated(id, key, engine, result));
                notify();
            });
        }
    }

    fn start_encodes(&mut self, log: &mut Vec<(bool, String)>) {
        let mut taken: Vec<PathBuf> = self
            .jobs
            .iter()
            .filter_map(|j| match &j.phase {
                Phase::Done(encoded) => Some(encoded.output.clone()),
                _ => None,
            })
            .collect();
        for index in 0..self.jobs.len() {
            let job = &self.jobs[index];
            if !job.wanted || job.phase != Phase::Idle {
                continue;
            }
            let Some(key) = self.key(job) else { continue };
            let estimate = match job.current_estimate(Some(&key)) {
                Some(Ok(estimate)) => *estimate,
                Some(Err(error)) => {
                    let message = format!("cannot encode without an estimate: {error}");
                    log.push((true, format!("{}: {message}", job.name())));
                    let job = &mut self.jobs[index];
                    job.wanted = false;
                    job.phase = Phase::Failed(message);
                    continue;
                }
                None => continue,
            };
            let Some(request) = self.request(job) else {
                continue;
            };
            let dir = self
                .settings
                .output_dir
                .clone()
                .or_else(|| job.path.parent().map(Path::to_path_buf))
                .unwrap_or_default();
            let name = naming::output_name(&job.path, &job.profile.suffix);
            let output = naming::unique_output(&dir, &name, &job.path, &taken);
            taken.push(output.clone());
            let (id, cancel) = (job.id, Arc::clone(&job.cancel));
            self.jobs[index].phase = Phase::Waiting;
            let sender = self.sender.clone();
            let notify = Arc::clone(&self.notify);
            let slots = Arc::clone(&self.encode_slots);
            std::thread::spawn(move || {
                let result = {
                    let _slot = slots.acquire();
                    if cancel.load(Ordering::Relaxed) {
                        Err(anyhow::anyhow!("cancelled"))
                    } else {
                        let _ = sender.send(Event::Started(id));
                        notify();
                        encode::run(&request, &estimate, &output, &cancel, |progress| {
                            let _ = sender.send(Event::Progress(id, progress));
                            notify();
                        })
                    }
                };
                let result = result.map_err(|e| {
                    let text = format!("{e:#}");
                    if text.ends_with("cancelled") {
                        "cancelled".to_owned()
                    } else {
                        text
                    }
                });
                let _ = sender.send(Event::Encoded(id, result));
                notify();
            });
        }
    }
}

/// Runs an estimate, falling back to CPU decoding then to the software filters when the GPU path fails
/// on this video. Returns the engine that worked (or the last one tried).
fn estimate_with_fallback(
    request: EstimateRequest,
    placebo: bool,
    slots: &Slots,
    cancel: &Arc<AtomicBool>,
) -> (Engine, Result<Estimate, String>) {
    let mut engines = vec![request.engine];
    if placebo && request.engine.vulkan_decode {
        engines.push(Engine {
            placebo: true,
            vulkan_decode: false,
        });
    }
    if request.engine.placebo {
        engines.push(Engine::SOFTWARE);
    }
    let mut last = (request.engine, Err("no engine".to_owned()));
    for engine in engines {
        let attempt = EstimateRequest {
            engine,
            ..request.clone()
        };
        match estimate::run(&attempt, slots, cancel) {
            Ok(estimate) => return (engine, Ok(estimate)),
            Err(error) => {
                let text = format!("{error:#}");
                if cancel.load(Ordering::Relaxed) || text.contains("cancelled") {
                    return (request.engine, Err("cancelled".to_owned()));
                }
                log::warn!("estimate with {} failed: {text}", engine.label());
                last = (engine, Err(text));
            }
        }
    }
    last
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicUsize;

    #[test]
    fn slots_bound_the_holders() {
        let slots = Arc::new(Slots::new(2));
        let current = Arc::new(AtomicUsize::new(0));
        let peak = Arc::new(AtomicUsize::new(0));
        let threads: Vec<_> = (0..8)
            .map(|_| {
                let (slots, current, peak) = (slots.clone(), current.clone(), peak.clone());
                std::thread::spawn(move || {
                    let _slot = slots.acquire();
                    let now = current.fetch_add(1, Ordering::SeqCst) + 1;
                    peak.fetch_max(now, Ordering::SeqCst);
                    std::thread::sleep(Duration::from_millis(20));
                    current.fetch_sub(1, Ordering::SeqCst);
                })
            })
            .collect();
        for thread in threads {
            thread.join().unwrap();
        }
        assert_eq!(peak.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn a_file_is_queued_once() {
        let mut queue = Queue::new(
            Capabilities::assumed(),
            QueueSettings::default(),
            Arc::new(|| {}),
        );
        let profile = Profile::messenger();
        let a = queue.add(PathBuf::from("does-not-exist.mp4"), &profile);
        let b = queue.add(PathBuf::from("does-not-exist.mp4"), &profile);
        assert_eq!(a, b);
        assert_eq!(queue.jobs.len(), 1);
    }
}
