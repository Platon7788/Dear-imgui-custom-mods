//! Bounded filesystem worker for hosts with a live rendering thread.

use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, SyncSender, TrySendError};

use super::*;

#[derive(Clone, Copy)]
pub(super) enum Navigation {
    Refresh,
    Push,
    Back,
    Forward,
}
struct Job {
    id: u64,
    path: PathBuf,
    navigation: Navigation,
    filter: FileFilter,
    files: bool,
    hidden: bool,
    probe: bool,
    operation: Option<super::operations::Operation>,
    sort: (SortColumn, SortOrder, bool),
}
struct Reply {
    job: Job,
    outcome: Outcome,
}
enum TargetState {
    Missing,
    File,
    Directory,
}
enum Outcome {
    Listing(Result<Vec<FsEntry>, String>),
    SaveTarget(Result<TargetState, String>),
    Mutation(Result<(), FmError>),
}
pub(super) struct BackgroundListing {
    tx: SyncSender<Job>,
    rx: Receiver<Reply>,
    queued: Option<Job>,
    generation: u64,
    pending: bool,
    mutating: bool,
    completion_error: Option<FmError>,
    pending_directory: Option<(PathBuf, Navigation)>,
}
impl BackgroundListing {
    fn new() -> Self {
        let (tx, jobs) = mpsc::sync_channel::<Job>(1);
        let (results, rx) = mpsc::sync_channel(1);
        std::thread::spawn(move || {
            while let Ok(mut job) = jobs.recv() {
                while let Ok(newer) = jobs.try_recv() {
                    job = newer;
                }
                let outcome = if let Some(operation) = &job.operation {
                    Outcome::Mutation(operation.execute())
                } else if job.probe {
                    Outcome::SaveTarget(match std::fs::metadata(&job.path) {
                        Ok(meta) if meta.is_dir() => Ok(TargetState::Directory),
                        Ok(_) => Ok(TargetState::File),
                        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                            Ok(TargetState::Missing)
                        }
                        Err(e) => Err(e.to_string()),
                    })
                } else {
                    let entries = std::fs::read_dir(&job.path)
                        .map(|dir| {
                            dir.flatten()
                                .filter_map(|e| FsEntry::from_dir_entry(&e))
                                .filter(|e| {
                                    (job.hidden || !e.is_hidden)
                                        && (e.is_dir
                                            || (job.files && job.filter.matches_ext(&e.extension)))
                                })
                                .collect::<Vec<_>>()
                        })
                        .map_err(|e| e.to_string())
                        .map(|mut entries| {
                            sort_entries(&mut entries, job.sort.0, job.sort.1, job.sort.2);
                            entries
                        });
                    Outcome::Listing(entries)
                };
                if results.send(Reply { job, outcome }).is_err() {
                    break;
                }
            }
        });
        Self {
            tx,
            rx,
            queued: None,
            generation: 0,
            pending: false,
            mutating: false,
            completion_error: None,
            pending_directory: None,
        }
    }
    pub(super) fn defer_completion_error(&mut self, error: FmError) -> bool {
        if self.pending_directory.is_none() {
            return false;
        }
        self.completion_error = Some(error);
        true
    }
    fn flush(&mut self) {
        if let Some(job) = self.queued.take() {
            match self.tx.try_send(job) {
                Ok(()) => {}
                Err(TrySendError::Full(job)) => self.queued = Some(job),
                Err(TrySendError::Disconnected(_)) => self.pending = false,
            }
        }
    }
}

impl FileManager {
    /// Index of the selected file type, including in SaveFile mode.
    pub fn active_filter_index(&self) -> usize {
        self.active_filter
    }

    /// Change the file type and update the filename before overwrite checks.
    pub fn set_active_filter(&mut self, index: usize) {
        if index < self.filters.len() {
            self.active_filter = index;
            self.normalize_save_extension();
            if self.is_open {
                self.refresh_directory();
            }
        }
    }

    /// Whether a background directory scan or save target check is in flight.
    pub fn listing_pending(&self) -> bool {
        self.background.as_ref().is_some_and(|b| b.pending)
    }

    pub(super) fn normalize_save_extension(&mut self) {
        if self.mode != DialogMode::SaveFile || !self.config.save_extension_from_filter {
            return;
        }
        let Some(filter) = self.filters.get(self.active_filter) else {
            return;
        };
        if filter.extensions.len() != 1 || self.filename_buf.trim().is_empty() {
            return;
        }
        // Validate before changing extensions: never turn an invalid path/device
        // name into an apparently valid name by replacing its suffix.
        if !util::is_valid_filename(self.filename_buf.trim()) {
            return;
        }
        let mut name = PathBuf::from(self.filename_buf.trim());
        name.set_extension(&filter.extensions[0]);
        self.filename_buf = name.to_string_lossy().into_owned();
    }

    pub(super) fn request_refresh(&mut self) {
        let (path, navigation) = self
            .background
            .as_ref()
            .and_then(|worker| worker.pending_directory.clone())
            .unwrap_or_else(|| (self.current_path.clone(), Navigation::Refresh));
        self.request_listing(path, navigation);
    }

    pub(super) fn request_listing(&mut self, path: PathBuf, navigation: Navigation) {
        let worker = self.background.get_or_insert_with(BackgroundListing::new);
        if worker.mutating {
            // Navigation can follow a mutation, but never replace its command.
            worker.pending_directory = Some((path, navigation));
            return;
        }
        worker.generation += 1;
        worker.pending = true;
        worker.pending_directory = Some((path.clone(), navigation));
        worker.queued = Some(Job {
            id: worker.generation,
            path,
            navigation,
            filter: self
                .filters
                .get(self.active_filter)
                .cloned()
                .unwrap_or_else(FileFilter::all),
            files: self.mode != DialogMode::SelectFolder,
            hidden: self.show_hidden,
            probe: false,
            operation: None,
            sort: (self.sort_column, self.sort_order, self.config.dirs_first),
        });
        worker.flush();
        self.loaded = true;
    }

    pub(super) fn request_save_check(&mut self, path: PathBuf) {
        let worker = self.background.get_or_insert_with(BackgroundListing::new);
        if worker.mutating {
            return;
        }
        worker.generation += 1;
        worker.pending = true;
        worker.pending_directory = None;
        worker.queued = Some(Job {
            id: worker.generation,
            path,
            navigation: Navigation::Refresh,
            filter: FileFilter::all(),
            files: false,
            hidden: false,
            probe: true,
            operation: None,
            sort: (self.sort_column, self.sort_order, self.config.dirs_first),
        });
        worker.flush();
    }

    pub(super) fn mutation_pending(&self) -> bool {
        self.background
            .as_ref()
            .is_some_and(|worker| worker.mutating)
    }

    pub(super) fn request_operation(&mut self, operation: super::operations::Operation) {
        let worker = self.background.get_or_insert_with(BackgroundListing::new);
        if worker.mutating {
            return;
        }
        worker.generation += 1;
        worker.pending = true;
        worker.mutating = true;
        worker.pending_directory = None;
        worker.queued = Some(Job {
            id: worker.generation,
            path: self.current_path.clone(),
            navigation: Navigation::Refresh,
            filter: FileFilter::all(),
            files: false,
            hidden: false,
            probe: false,
            operation: Some(operation),
            sort: (self.sort_column, self.sort_order, self.config.dirs_first),
        });
        worker.flush();
    }

    pub(super) fn poll_listing(&mut self) {
        let Some(worker) = self.background.as_mut() else {
            return;
        };
        worker.flush();
        let mut mutation = None;
        while let Ok(reply) = worker.rx.try_recv() {
            if reply.job.id != worker.generation {
                continue;
            }
            worker.pending = false;
            if reply.job.operation.is_none() {
                worker.pending_directory = None;
            }
            match reply.outcome {
                Outcome::Mutation(result) => {
                    worker.mutating = false;
                    mutation = Some(result);
                }
                Outcome::SaveTarget(Ok(TargetState::Directory)) => {
                    self.error = Some(FmError::CreateFileFailed(
                        self.config.strings.target_is_directory.to_string(),
                    ))
                }
                Outcome::SaveTarget(Err(e)) => self.error = Some(FmError::CreateFileFailed(e)),
                Outcome::SaveTarget(Ok(TargetState::File)) => {
                    self.overwrite_target = Some(reply.job.path);
                    self.show_overwrite_confirm = true;
                }
                Outcome::SaveTarget(Ok(TargetState::Missing)) => {
                    self.selected_path = Some(reply.job.path);
                    self.save_ready = true;
                }
                Outcome::Listing(Err(e)) => self.error = Some(FmError::CannotReadDir(e)),
                Outcome::Listing(Ok(entries)) => {
                    let previous = self.current_path.clone();
                    if self.config.enable_history {
                        match reply.job.navigation {
                            Navigation::Push => self.history.push(&previous),
                            Navigation::Back => {
                                self.history.go_back(&previous);
                            }
                            Navigation::Forward => {
                                self.history.go_forward(&previous);
                            }
                            Navigation::Refresh => {}
                        }
                    }
                    self.current_path = reply.job.path;
                    self.entries = entries;
                    self.selected_indices.clear();
                    self.last_click_index = None;
                    self.scroll_to_index = None;
                    self.rename_index = None;
                    self.rename_buf.clear();
                    self.context_menu_target = None;
                    self.delete_target = None;
                    self.show_delete_confirm = false;
                    self.error = worker.completion_error.take();
                    self.path_input_buf = self.current_path.to_string_lossy().into_owned();
                    rebuild_breadcrumb_segments(&self.current_path, &mut self.breadcrumb_segments);
                }
            }
        }
        if let Some(result) = mutation {
            self.finish_operation(result);
        }
    }
}

#[cfg(test)]
#[path = "background_tests.rs"]
mod tests;
