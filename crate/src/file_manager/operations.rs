//! Filesystem operations with frozen path identities, shared by sync and worker modes.
use super::{FileManager, FmError};
use std::path::PathBuf;

pub(super) enum Operation {
    CreateFolder(PathBuf),
    CreateFile(PathBuf),
    Rename {
        old: PathBuf,
        new: PathBuf,
        target_exists: &'static str,
    },
    Delete(PathBuf),
}
impl Operation {
    pub(super) fn execute(&self) -> Result<(), FmError> {
        match self {
            Self::CreateFolder(path) => {
                std::fs::create_dir(path).map_err(|e| FmError::CreateFolderFailed(e.to_string()))
            }
            Self::CreateFile(path) => std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(path)
                .map(|_| ())
                .map_err(|e| FmError::CreateFileFailed(e.to_string())),
            Self::Rename {
                old,
                new,
                target_exists,
            } => {
                if new != old && new.symlink_metadata().is_ok() {
                    return Err(FmError::RenameFailed(format!(
                        "{target_exists}: \"{}\"",
                        new.display()
                    )));
                }
                std::fs::rename(old, new).map_err(|e| FmError::RenameFailed(e.to_string()))
            }
            Self::Delete(path) => {
                let result = match std::fs::symlink_metadata(path) {
                    Ok(meta) if meta.file_type().is_symlink() => {
                        #[cfg(windows)]
                        {
                            std::fs::remove_dir(path).or_else(|_| std::fs::remove_file(path))
                        }
                        #[cfg(not(windows))]
                        {
                            std::fs::remove_file(path)
                        }
                    }
                    Ok(meta) if meta.is_dir() => std::fs::remove_dir_all(path),
                    Ok(_) => std::fs::remove_file(path),
                    Err(e) => Err(e),
                };
                result.map_err(|e| FmError::DeleteFailed(e.to_string()))
            }
        }
    }
}
impl FileManager {
    pub(super) fn run_operation(&mut self, operation: Operation) {
        if self.config.background_listing {
            self.request_operation(operation);
        } else {
            self.finish_operation(operation.execute());
        }
    }
    pub(super) fn finish_operation(&mut self, result: Result<(), FmError>) {
        match result {
            Ok(()) => {
                self.show_new_folder = false;
                self.show_new_file = false;
                self.new_folder_buf.clear();
                self.new_file_buf.clear();
                self.rename_index = None;
                self.rename_buf.clear();
                self.delete_target = None;
                self.show_delete_confirm = false;
                self.refresh_directory();
            }
            Err(error) => {
                if let Some(worker) = self.background.as_mut()
                    && worker.defer_completion_error(error.clone())
                {
                    self.refresh_directory();
                } else {
                    self.error = Some(error);
                }
            }
        }
    }
}
