/* SPDX-License-Identifier: MPL-2.0 */
//! Foundation item removal; POSIX unlink keeps its nonrecursive semantics.
use super::{FileLocation, Fs, FsError, FsNode, GuestPath};
use std::path::Path;

fn plain_path(path: &Path) -> Result<std::path::PathBuf, FsError> {
    let metadata = std::fs::symlink_metadata(path).map_err(FsError::IoError)?;
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if metadata.file_attributes() & 0x400 != 0 {
            return Err(FsError::AccessDenied);
        }
    }
    if metadata.file_type().is_symlink() {
        return Err(FsError::AccessDenied);
    }
    std::fs::canonicalize(path).map_err(FsError::IoError)
}

impl Fs {
    /// Remove a Foundation item, including a directory's descendants. Resolve
    /// everything through the VFS and preflight the complete writable subtree
    /// before removing any entries. Never traverse host links or other mounts.
    pub fn remove_recursively(&mut self, path: &GuestPath) -> Result<(), FsError> {
        // Checkpoint pruning removes ordinary files frequently. The primitive
        // already enforces their VFS permissions and never traverses a link.
        if self.is_file(path) {
            return self.remove(path);
        }
        {
            let (parent, name) = self
                .lookup_parent_node(path)
                .ok_or(FsError::NonexistentParentDir)?;
            let FsNode::Directory {
                children,
                writeable,
            } = parent
            else {
                return Err(FsError::InvalidParentDir);
            };
            let parent = plain_path(writeable.as_ref().ok_or(FsError::ReadonlyParentDir)?)?;
            let node = children.get(&name).ok_or(FsError::DoesNotExist)?;
            let root = match node {
                FsNode::Directory {
                    writeable: Some(host),
                    ..
                } => plain_path(host)?,
                FsNode::File {
                    location: FileLocation::Path(host),
                    writeable: true,
                } => plain_path(host)?,
                _ => return Err(FsError::AccessDenied),
            };
            if root.parent() != Some(parent.as_path()) {
                return Err(FsError::AccessDenied);
            }
            let mut pending = vec![node];
            while let Some(node) = pending.pop() {
                let host = match node {
                    FsNode::Directory {
                        children,
                        writeable: Some(host),
                    } => {
                        pending.extend(children.values());
                        host
                    }
                    FsNode::File {
                        location: FileLocation::Path(host),
                        writeable: true,
                    } => host,
                    _ => return Err(FsError::AccessDenied),
                };
                if !plain_path(host)?.starts_with(&root) {
                    return Err(FsError::AccessDenied);
                }
            }
        }
        if self.is_dir(path) {
            let descendants = self
                .enumerate_recursive(path)
                .map_err(|_| FsError::DoesNotExist)?;
            // enumerate_recursive emits a directory before its children.
            for child in descendants.into_iter().rev() {
                self.remove(path.join(child.as_str()))?;
            }
        }
        self.remove(path)
    }
}

#[cfg(test)]
mod tests;
