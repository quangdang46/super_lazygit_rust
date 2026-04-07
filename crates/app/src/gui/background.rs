// Ported from ./references/lazygit-master/pkg/gui/background.go

use std::sync::mpsc;
use std::time::Duration;

/// Manages background routines for the GUI including auto-fetch and auto-refresh
pub struct BackgroundRoutineMgr {
    /// If we've suspended the gui (e.g. because we've switched to a subprocess)
    /// we typically want to pause some things that are running like background file refreshes
    pause_background_refreshes: bool,

    /// A channel to trigger an immediate background fetch; we use this when switching repos
    trigger_fetch: Option<mpsc::Sender<()>>,
}

impl BackgroundRoutineMgr {
    pub fn new() -> Self {
        Self {
            pause_background_refreshes: false,
            trigger_fetch: None,
        }
    }

    pub fn pause_background_refreshes(&mut self, pause: bool) {
        self.pause_background_refreshes = pause;
    }

    pub fn trigger_immediate_fetch(&self) {
        if let Some(ref sender) = self.trigger_fetch {
            let _ = sender.send(());
        }
    }

    pub fn start_background_routines(&mut self) {
        // Background fetch and refresh would be started here
        // In a full implementation, this would spawn goroutines via utils::safe
    }

    pub fn go_every<F>(&mut self, interval: Duration, stop: mpsc::Receiver<()>, mut function: F)
    where
        F: FnMut(bool) -> Result<(), String> + Send + 'static,
    {
        // Placeholder for goEvery functionality
        // Would create ticker and handle retrigger channel
    }
}

impl Default for BackgroundRoutineMgr {
    fn default() -> Self {
        Self::new()
    }
}
