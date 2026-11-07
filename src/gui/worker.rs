//! Background analysis thread and the message channel to the UI.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, channel};

use eframe::egui;

use crate::report::{self, Progress, Report, ScanOptions};

/// Messages sent from the worker thread to the UI.
pub enum WorkerMsg {
    Progress(Progress),
    Finished(Box<Report>),
}

/// Handle to a running analysis.
pub struct AnalysisWorker {
    rx: Receiver<WorkerMsg>,
    cancel: Arc<AtomicBool>,
}

impl AnalysisWorker {
    /// Start analyzing in a background thread.
    pub fn spawn(ctx: egui::Context, options: ScanOptions) -> Self {
        let (tx, rx) = channel();
        let cancel = Arc::new(AtomicBool::new(false));
        let thread_cancel = Arc::clone(&cancel);
        let progress_tx = tx.clone();

        std::thread::spawn(move || {
            let report = report::analyze(&options, Some(&thread_cancel), |progress| {
                let _ = progress_tx.send(WorkerMsg::Progress(progress));
                ctx.request_repaint();
            });
            let _ = tx.send(WorkerMsg::Finished(Box::new(report)));
            ctx.request_repaint();
        });

        Self { rx, cancel }
    }

    /// Drain all pending messages without blocking.
    pub fn poll(&self) -> Vec<WorkerMsg> {
        let mut messages = Vec::new();
        while let Ok(message) = self.rx.try_recv() {
            messages.push(message);
        }
        messages
    }

    /// Ask the worker to stop as soon as possible.
    pub fn cancel(&self) {
        self.cancel.store(true, Ordering::Relaxed);
    }
}
