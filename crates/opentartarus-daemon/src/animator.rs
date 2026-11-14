//! Plays a custom frame's animation by drawing it in a background thread
//! and writing each frame to razerkbd's sysfs nodes. D-Bus is too slow for
//! 30 frames a second; sysfs takes them easily.

use opentartarus_core::animation::{
    render, Animation, Press, FRAME_INTERVAL_MS, PRESS_MEMORY_SECS,
};
use opentartarus_core::lighting::custom_frame_payload;
use opentartarus_core::types::LightFrame;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Condvar, Mutex, OnceLock};
use std::time::{Duration, Instant};

/// Writes in a row that may fail before the animation gives up, so an
/// unplugged pad doesn't keep the thread busy.
const MAX_FAILED_WRITES: u32 = 30;

struct Job {
    node: PathBuf,
    frame: LightFrame,
    animation: Animation,
    started: Instant,
    failed_writes: u32,
}

#[derive(Default)]
struct Shared {
    job: Option<Job>,
    presses: Vec<(usize, usize, Instant)>,
}

pub struct Animator {
    shared: Mutex<Shared>,
    wake: Condvar,
}

/// The one animator, with its thread started on first use.
pub fn animator() -> &'static Animator {
    static ANIMATOR: OnceLock<&'static Animator> = OnceLock::new();
    ANIMATOR.get_or_init(|| {
        let animator: &'static Animator = Box::leak(Box::new(Animator {
            shared: Mutex::new(Shared::default()),
            wake: Condvar::new(),
        }));
        std::thread::Builder::new()
            .name("animator".into())
            .spawn(move || animator.run())
            .expect("spawn animator thread");
        animator
    })
}

impl Animator {
    /// Starts animating `frame` on the pad at `node`, replacing whatever
    /// played before.
    pub fn play(&self, node: PathBuf, frame: LightFrame, animation: Animation) {
        let mut shared = self.shared.lock().expect("animator");
        shared.job = Some(Job {
            node,
            frame,
            animation,
            started: Instant::now(),
            failed_writes: 0,
        });
        self.wake.notify_all();
    }

    /// Stops animating. Once this returns, no further frame is written, so a
    /// lighting change made right after it is not drawn over.
    pub fn stop(&self) {
        self.shared.lock().expect("animator").job = None;
    }

    /// Records a key going down, for the animations that react to presses.
    pub fn key_pressed(&self, row: usize, column: usize) {
        let mut shared = self.shared.lock().expect("animator");
        if shared
            .job
            .as_ref()
            .is_some_and(|job| job.animation.kind.uses_presses())
        {
            shared.presses.push((row, column, Instant::now()));
        }
    }

    fn run(&self) {
        let mut shared = self.shared.lock().expect("animator");
        loop {
            while shared.job.is_none() {
                shared.presses.clear();
                shared = self.wake.wait(shared).expect("animator");
            }
            let now = Instant::now();
            shared
                .presses
                .retain(|(_, _, at)| now.duration_since(*at).as_secs_f32() < PRESS_MEMORY_SECS);
            let presses: Vec<Press> = shared
                .presses
                .iter()
                .map(|(row, column, at)| Press {
                    row: *row,
                    column: *column,
                    age: now.duration_since(*at).as_secs_f32(),
                })
                .collect();
            if let Some(job) = shared.job.as_mut() {
                let t = now.duration_since(job.started).as_secs_f32();
                let frame = render(&job.frame, job.animation, t, &presses);
                // Written under the lock, so `stop` can't return while a
                // stale frame is still on its way to the pad.
                if write_frame(&job.node, &frame) {
                    job.failed_writes = 0;
                } else {
                    job.failed_writes += 1;
                    if job.failed_writes >= MAX_FAILED_WRITES {
                        crate::log::write("animation stopped: the pad's lighting node is gone");
                        shared.job = None;
                        continue;
                    }
                }
            }
            let (next, _) = self
                .wake
                .wait_timeout(shared, Duration::from_millis(FRAME_INTERVAL_MS))
                .expect("animator");
            shared = next;
        }
    }
}

fn write_frame(node: &Path, frame: &LightFrame) -> bool {
    fs::write(
        node.join("matrix_custom_frame"),
        custom_frame_payload(frame),
    )
    .is_ok()
        && fs::write(node.join("matrix_effect_custom"), b"1").is_ok()
}
