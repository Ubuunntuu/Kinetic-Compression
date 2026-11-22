use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Condvar, Mutex};
use std::time::Instant;
use serde::{Deserialize, Serialize};
use sysinfo::System;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum JobState {
    Idle,
    Running,
    Paused,
    Cancelled,
    Completed,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProgressSnapshot {
    pub state: JobState,
    pub current_operation: String,
    pub current_file: String,
    pub processed_bytes: u64,
    pub total_bytes: u64,
    pub compressed_bytes: u64,
    pub progress_percent: f64,
    pub compression_ratio_percent: f64,
    pub throughput_mb_s: f64,
    pub elapsed_ms: u64,
    pub estimated_remaining_ms: u64,
    pub active_threads: usize,
    pub cpu_usage_percent: f32,
    pub memory_used_mb: f64,
    pub arena_used_mb: f64,
    pub arena_peak_mb: f64,
}

pub struct JobController {
    state: Mutex<JobState>,
    current_operation: Mutex<String>,
    current_file: Mutex<String>,
    processed_bytes: AtomicU64,
    total_bytes: AtomicU64,
    compressed_bytes: AtomicU64,
    active_threads: AtomicUsize,
    started_at: Mutex<Option<Instant>>,
    accumulated_ms: AtomicU64,
    paused: AtomicBool,
    cancelled: AtomicBool,
    pause_lock: Mutex<()>,
    pause_cv: Condvar,
}

impl Default for JobController {
    fn default() -> Self {
        Self::new()
    }
}

impl JobController {
    pub fn new() -> Self {
        Self {
            state: Mutex::new(JobState::Idle),
            current_operation: Mutex::new("Idle".to_string()),
            current_file: Mutex::new("None".to_string()),
            processed_bytes: AtomicU64::new(0),
            total_bytes: AtomicU64::new(0),
            compressed_bytes: AtomicU64::new(0),
            active_threads: AtomicUsize::new(0),
            started_at: Mutex::new(None),
            accumulated_ms: AtomicU64::new(0),
            paused: AtomicBool::new(false),
            cancelled: AtomicBool::new(false),
            pause_lock: Mutex::new(()),
            pause_cv: Condvar::new(),
        }
    }

    pub fn begin_job(&self, operation: &str, total_bytes: u64, worker_threads: usize) {
        *self.state.lock().unwrap() = JobState::Running;
        *self.current_operation.lock().unwrap() = operation.to_string();
        *self.current_file.lock().unwrap() = "Initializing pipeline...".to_string();
        self.processed_bytes.store(0, Ordering::SeqCst);
        self.total_bytes.store(total_bytes, Ordering::SeqCst);
        self.compressed_bytes.store(0, Ordering::SeqCst);
        self.active_threads.store(worker_threads, Ordering::SeqCst);
        self.accumulated_ms.store(0, Ordering::SeqCst);
        self.paused.store(false, Ordering::SeqCst);
        self.cancelled.store(false, Ordering::SeqCst);
        *self.started_at.lock().unwrap() = Some(Instant::now());
    }

    pub fn set_current_file(&self, file_name: &str) {
        *self.current_file.lock().unwrap() = file_name.to_string();
    }

    pub fn record_block_progress(&self, raw_bytes: u64, compressed_bytes: u64) {
        self.processed_bytes.fetch_add(raw_bytes, Ordering::SeqCst);
        self.compressed_bytes
            .fetch_add(compressed_bytes, Ordering::SeqCst);
    }

    pub fn pause(&self) {
        let mut st = self.state.lock().unwrap();
        if *st == JobState::Running {
            *st = JobState::Paused;
            self.paused.store(true, Ordering::SeqCst);
            let mut started = self.started_at.lock().unwrap();
            if let Some(t) = started.take() {
                self.accumulated_ms
                    .fetch_add(t.elapsed().as_millis() as u64, Ordering::SeqCst);
            }
        }
    }

    pub fn resume(&self) {
        let mut st = self.state.lock().unwrap();
        if *st == JobState::Paused {
            *st = JobState::Running;
            self.paused.store(false, Ordering::SeqCst);
            *self.started_at.lock().unwrap() = Some(Instant::now());
            self.pause_cv.notify_all();
        }
    }

    pub fn cancel(&self) {
        *self.state.lock().unwrap() = JobState::Cancelled;
        self.cancelled.store(true, Ordering::SeqCst);
        self.paused.store(false, Ordering::SeqCst);
        self.active_threads.store(0, Ordering::SeqCst);
        self.pause_cv.notify_all();
    }

    pub fn finish_job(&self) {
        let mut st = self.state.lock().unwrap();
        if *st != JobState::Cancelled {
            *st = JobState::Completed;
        }
        let mut started = self.started_at.lock().unwrap();
        if let Some(t) = started.take() {
            self.accumulated_ms
                .fetch_add(t.elapsed().as_millis() as u64, Ordering::SeqCst);
        }
        self.active_threads.store(0, Ordering::SeqCst);
    }

    pub fn fail_job(&self, reason: &str) {
        *self.state.lock().unwrap() = JobState::Failed;
        *self.current_file.lock().unwrap() = format!("Error: {}", reason);
        self.active_threads.store(0, Ordering::SeqCst);
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Relaxed)
    }

    pub fn check_point(&self) -> Result<(), String> {
        if self.cancelled.load(Ordering::SeqCst) {
            return Err("Operation cancelled by user.".to_string());
        }
        if self.paused.load(Ordering::SeqCst) {
            let mut guard = self.pause_lock.lock().unwrap();
            while self.paused.load(Ordering::SeqCst) && !self.cancelled.load(Ordering::SeqCst) {
                guard = self.pause_cv.wait(guard).unwrap();
            }
            drop(guard);
        }
        if self.cancelled.load(Ordering::SeqCst) {
            return Err("Operation cancelled by user.".to_string());
        }
        Ok(())
    }

    pub fn snapshot(&self, arena_used_bytes: u64, arena_peak_bytes: u64) -> ProgressSnapshot {
        let state = *self.state.lock().unwrap();
        let current_operation = self.current_operation.lock().unwrap().clone();
        let current_file = self.current_file.lock().unwrap().clone();
        let processed = self.processed_bytes.load(Ordering::SeqCst);
        let total = self.total_bytes.load(Ordering::SeqCst);
        let compressed = self.compressed_bytes.load(Ordering::SeqCst);
        let active_threads = self.active_threads.load(Ordering::SeqCst);

        let running_ms = self
            .started_at
            .lock()
            .unwrap()
            .map(|t| t.elapsed().as_millis() as u64)
            .unwrap_or(0);
        let elapsed_ms = self.accumulated_ms.load(Ordering::SeqCst) + running_ms;

        let progress_percent = if total > 0 {
            ((processed as f64) / (total as f64) * 100.0).clamp(0.0, 100.0)
        } else {
            0.0
        };

        let compression_ratio_percent = if processed > 0 {
            ((compressed as f64) / (processed as f64)) * 100.0
        } else {
            100.0
        };

        let throughput_mb_s = if elapsed_ms > 0 {
            ((processed as f64) / (1024.0 * 1024.0)) / ((elapsed_ms as f64) / 1000.0)
        } else {
            0.0
        };

        let estimated_remaining_ms = if processed > 0 && total > processed && elapsed_ms > 0 {
            let remaining_bytes = total - processed;
            ((remaining_bytes as f64) / (processed as f64) * (elapsed_ms as f64)) as u64
        } else {
            0
        };

        let mut sys = System::new();
        sys.refresh_cpu();
        sys.refresh_memory();
        let cpu_usage_percent = sys.global_cpu_info().cpu_usage();
        let memory_used_mb = (sys.used_memory() as f64) / (1024.0 * 1024.0);

        ProgressSnapshot {
            state,
            current_operation,
            current_file,
            processed_bytes: processed,
            total_bytes: total,
            compressed_bytes: compressed,
            progress_percent: (progress_percent * 10.0).round() / 10.0,
            compression_ratio_percent: (compression_ratio_percent * 10.0).round() / 10.0,
            throughput_mb_s: (throughput_mb_s * 100.0).round() / 100.0,
            elapsed_ms,
            estimated_remaining_ms,
            active_threads,
            cpu_usage_percent: (cpu_usage_percent * 10.0).round() / 10.0,
            memory_used_mb: (memory_used_mb * 10.0).round() / 10.0,
            arena_used_mb: ((arena_used_bytes as f64) / (1024.0 * 1024.0) * 100.0).round() / 100.0,
            arena_peak_mb: ((arena_peak_bytes as f64) / (1024.0 * 1024.0) * 100.0).round() / 100.0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_job_controller_pause_resume_and_cancel() {
        let ctrl = JobController::new();
        ctrl.begin_job("Test Compression", 10 * 1024 * 1024, 4);
        assert!(ctrl.check_point().is_ok());

        ctrl.record_block_progress(5 * 1024 * 1024, 2 * 1024 * 1024);
        let snap = ctrl.snapshot(4 * 1024 * 1024, 8 * 1024 * 1024);
        assert_eq!(snap.state, JobState::Running);
        assert_eq!(snap.progress_percent, 50.0);
        assert_eq!(snap.compression_ratio_percent, 40.0);

        ctrl.pause();
        assert_eq!(ctrl.snapshot(0, 0).state, JobState::Paused);

        ctrl.resume();
        assert_eq!(ctrl.snapshot(0, 0).state, JobState::Running);

        ctrl.cancel();
        assert!(ctrl.check_point().is_err());
        assert_eq!(ctrl.snapshot(0, 0).state, JobState::Cancelled);
    }
}