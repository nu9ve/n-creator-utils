//! Reads below a registered root without following links. Port of the read side of `media/common.py` `Tree`
//! (`read_bytes`, `stat`). Python walks with directory descriptors and `O_NOFOLLOW`; here every component is
//! stamped without following it, and a link, junction or reparse point anywhere is refused, as Python refuses one.

use crate::rules::relative;
use nu9ve_json::{Error, Result, require};
use nu9ve_volumes::{EntryKind, Stamp, fs, stamp};
use std::path::{Path, PathBuf};

fn io(error: std::io::Error) -> Error {
    Error::new("operation_failed", error.to_string())
}

/// The root, checked as `Tree.__enter__`: no link in it or its parents, and a directory.
fn open_root(root: &Path) -> Result<PathBuf> {
    let root = std::path::absolute(root).map_err(io)?;
    for parent in root.ancestors() {
        if let Ok(s) = stamp(parent) {
            require(
                s.kind != EntryKind::Link,
                "unsafe_path",
                format!("Symlink in root path: {}", parent.display()),
            )?;
        }
    }
    let s = stamp(&root).map_err(|e| Error::new("operation_failed", e.message))?;
    require(
        s.kind == EntryKind::Dir,
        "operation_failed",
        format!("Not a directory: {}", root.display()),
    )?;
    Ok(root)
}

/// The stamp of `rel` below `root`, or `None` when it (or a parent folder) does not exist. Links refused.
fn walk(root: &Path, rel: &str) -> Result<Option<(PathBuf, Stamp)>> {
    let root = open_root(root)?;
    let parts: Vec<&str> = relative(rel)?.split('/').collect();
    let mut path = root;
    for (i, part) in parts.iter().enumerate() {
        path.push(part);
        let Ok(s) = stamp(&path) else { return Ok(None) };
        require(
            s.kind != EntryKind::Link,
            "operation_failed",
            format!("Too many levels of symbolic links: {rel}"),
        )?;
        if i + 1 < parts.len() {
            if s.kind != EntryKind::Dir {
                return Ok(None); // NotADirectoryError
            }
        } else {
            require(
                s.kind == EntryKind::File,
                "unsafe_file",
                format!("Not a regular file: {rel}"),
            )?;
            return Ok(Some((path, s)));
        }
    }
    Ok(None)
}

/// Python `Tree(root).read_bytes(rel, limit)`: the whole small file, or `None` when absent.
pub fn read_bytes(root: &Path, rel: &str, limit: usize) -> Result<Option<Vec<u8>>> {
    let Some((path, _)) = walk(root, rel)? else {
        return Ok(None);
    };
    use std::io::Read;
    let mut data = Vec::new();
    std::fs::File::open(&path)
        .map_err(io)?
        .take(limit as u64 + 1)
        .read_to_end(&mut data)
        .map_err(io)?;
    require(
        data.len() <= limit,
        "file_too_large",
        format!("File too large: {rel}"),
    )?;
    Ok(Some(data))
}

/// Python `Tree(root).stat(rel)` with `FileNotFoundError` as `None`: size of a regular file.
pub fn file_size(root: &Path, rel: &str) -> Result<Option<u64>> {
    Ok(walk(root, rel)?.map(|(_, s)| s.size))
}

/// Python `content_stamp`: the JSON stamp the plan and the receipts carry.
pub fn stamp_json(s: &Stamp) -> serde_json::Value {
    serde_json::json!({"size": s.size, "mtime_ns": s.mtime_ns, "ctime_ns": s.ctime_ns, "inode": s.inode})
}

/// Whether an error is Python's `FileNotFoundError` or `NotADirectoryError`.
pub fn absent(error: &Error) -> bool {
    fs::python_exception(error)
        .is_some_and(|n| n == "FileNotFoundError" || n == "NotADirectoryError")
}

/// The write side of the Python `Tree`: an open root whose identity is rechecked before each step, folders walked
/// without following links, files created exclusively, renamed without replacing and synced with their folder.
pub struct Root {
    pub path: PathBuf,
    inode: u64,
}

impl Root {
    pub fn open(root: &Path) -> Result<Root> {
        let path = open_root(root)?;
        let inode = stamp(&path)
            .map_err(|e| Error::new("operation_failed", e.message))?
            .inode;
        Ok(Root { path, inode })
    }

    /// Python `Tree.check`: the root was not replaced or disconnected since it was opened.
    pub fn check(&self) -> Result<()> {
        let same = fs::stamp_opt(&self.path)?
            .is_some_and(|s| s.kind == EntryKind::Dir && s.inode == self.inode);
        require(
            same,
            "root_changed",
            format!(
                "Root directory was replaced or disconnected: {}",
                self.path.display()
            ),
        )
    }

    /// Python `Tree.parent(rel, create)`: the folder that holds `rel`, created on demand; never through a link.
    pub fn parent(&self, rel: &str, create: bool) -> Result<(PathBuf, String)> {
        self.check()?;
        let parts: Vec<&str> = relative(rel)?.split('/').collect();
        let mut dir = self.path.clone();
        for part in &parts[..parts.len() - 1] {
            let next = dir.join(part);
            match fs::stamp_opt(&next)? {
                Some(s) if s.kind == EntryKind::Dir => {}
                Some(_) => {
                    return Err(Error::new(
                        "operation_failed",
                        format!("[NotADirectoryError] {}", next.display()),
                    ));
                }
                None if create => {
                    match std::fs::create_dir(&next) {
                        Ok(()) => {
                            #[cfg(unix)]
                            {
                                use std::os::unix::fs::PermissionsExt;
                                let mode = std::fs::Permissions::from_mode(0o700);
                                let _ = std::fs::set_permissions(&next, mode);
                            }
                            fs::sync_dir(&dir)?;
                        }
                        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
                        Err(e) => return Err(fs::io_error(e, &next)),
                    }
                    require(
                        fs::is_dir(&next)? == Some(true),
                        "operation_failed",
                        format!("Not a directory: {}", next.display()),
                    )?;
                }
                None => {
                    return Err(Error::new(
                        "operation_failed",
                        format!("[FileNotFoundError] {}", next.display()),
                    ));
                }
            }
            dir = next;
        }
        Ok((dir, parts[parts.len() - 1].to_owned()))
    }

    /// Python `Tree.open(rel)` for reading: a regular file, or `None` when it (or a folder) is absent.
    pub fn open_read(&self, rel: &str) -> Result<Option<std::fs::File>> {
        match self.parent(rel, false) {
            Ok((dir, name)) => fs::open_read(&dir.join(name)),
            Err(e) if absent(&e) => Ok(None),
            Err(e) => Err(e),
        }
    }

    /// A new file at `rel` (`O_EXCL`), creating its folders.
    pub fn create_new(&self, rel: &str) -> Result<std::fs::File> {
        let (dir, name) = self.parent(rel, true)?;
        fs::create_new(&dir.join(name))
    }

    /// Python `Tree.stat(rel)` with `FileNotFoundError` as `None`; a link or folder is refused.
    pub fn stat(&self, rel: &str) -> Result<Option<Stamp>> {
        let (dir, name) = match self.parent(rel, false) {
            Ok(found) => found,
            Err(e) if absent(&e) => return Ok(None),
            Err(e) => return Err(e),
        };
        let Some(s) = fs::stamp_opt(&dir.join(name))? else {
            return Ok(None);
        };
        require(
            s.kind == EntryKind::File,
            "unsafe_file",
            format!("Not a regular file: {rel}"),
        )?;
        Ok(Some(s))
    }

    /// Python `Tree.unlink(rel, expected)`: a regular file, unchanged when a stamp is given; the folder is synced.
    pub fn unlink(&self, rel: &str, expected: Option<&serde_json::Value>) -> Result<()> {
        let (dir, name) = self.parent(rel, false)?;
        let path = dir.join(&name);
        let s = fs::stamp_opt(&path)?
            .ok_or_else(|| Error::new("operation_failed", format!("[FileNotFoundError] {rel}")))?;
        require(
            s.kind == EntryKind::File,
            "unsafe_file",
            format!("Not a regular file: {rel}"),
        )?;
        if let Some(expected) = expected {
            require(
                &stamp_json(&s) == expected,
                "source_changed",
                format!("File changed before cleanup: {rel}"),
            )?;
        }
        std::fs::remove_file(&path).map_err(|e| fs::io_error(e, &path))?;
        fs::sync_dir(&dir)
    }

    /// Rename within this root without replacing a file, syncing both parent folders.
    pub fn rename_new(&self, from: &str, to: &str) -> Result<()> {
        let (dir, from_name) = self.parent(from, false)?;
        let (destination, to_name) = self.parent(to, false)?;
        fs::rename_new(&dir.join(from_name), &destination.join(to_name))?;
        fs::sync_dir(&dir)?;
        if destination != dir {
            fs::sync_dir(&destination)?;
        }
        Ok(())
    }

    /// Sync the folder that holds `rel`.
    pub fn sync_parent(&self, rel: &str) -> Result<()> {
        let (dir, _) = self.parent(rel, false)?;
        fs::sync_dir(&dir)
    }

    /// Python `Tree.write_bytes(rel, data, replace)`: a durable whole-file write through a temporary file.
    pub fn write_bytes(&self, rel: &str, data: &[u8], replace: bool) -> Result<()> {
        use std::io::Write;
        let (dir, name) = self.parent(rel, true)?;
        let temp = dir.join(format!(".{name}.{}.tmp", nu9ve_json::uid()?));
        let result = (|| -> Result<()> {
            let mut out = fs::create_new(&temp)?;
            out.write_all(data).map_err(|e| fs::io_error(e, &temp))?;
            fs::full_sync(&out)?;
            drop(out);
            let target = dir.join(&name);
            if let Some(old) = fs::stamp_opt(&target)? {
                require(
                    old.kind == EntryKind::File,
                    "unsafe_file",
                    "Unsafe metadata target",
                )?;
                require(
                    replace,
                    "destination_exists",
                    format!("Already exists: {rel}"),
                )?;
                fs::replace(&temp, &target)?;
            } else {
                fs::rename_new(&temp, &target)?;
            }
            fs::sync_dir(&dir)
        })();
        let _ = std::fs::remove_file(&temp);
        result
    }

    /// Python `Tree.write_json`: canonical JSON and a newline.
    pub fn write_json(&self, rel: &str, value: &serde_json::Value) -> Result<()> {
        let mut data = nu9ve_json::canonical(value);
        data.push(b'\n');
        self.write_bytes(rel, &data, true)
    }
}

#[cfg(test)]
mod root_tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn writes_are_exclusive_durable_and_never_replace_media() {
        let dir = tempfile::tempdir().unwrap();
        let root = Root::open(dir.path()).unwrap();
        root.create_new("a/b/.c.ncpartial")
            .unwrap()
            .write_all(b"bytes")
            .unwrap();
        assert!(root.create_new("a/b/.c.ncpartial").is_err());
        root.rename_new("a/b/.c.ncpartial", "a/b/c").unwrap();
        assert_eq!(root.stat("a/b/c").unwrap().unwrap().size, 5);
        root.create_new("a/b/.d").unwrap();
        assert_eq!(
            root.rename_new("a/b/.d", "a/b/c").unwrap_err().code,
            "destination_conflict"
        );
        assert!(root.stat("a/x/y").unwrap().is_none());
        assert!(root.open_read("a/b/zz").unwrap().is_none());
        root.write_json(
            ".ncreator/receipts/j.json",
            &serde_json::json!({"b": 1, "a": 2}),
        )
        .unwrap();
        root.write_json(".ncreator/receipts/j.json", &serde_json::json!({"a": 3}))
            .unwrap();
        assert_eq!(
            std::fs::read(dir.path().join(".ncreator/receipts/j.json")).unwrap(),
            b"{\"a\":3}\n"
        );
        assert_eq!(
            root.write_bytes("a/b/c", b"x", false).unwrap_err().code,
            "destination_exists"
        );
        let s = root.stat("a/b/c").unwrap().unwrap();
        let mut other = stamp_json(&s);
        other["size"] = serde_json::json!(1);
        assert_eq!(
            root.unlink("a/b/c", Some(&other)).unwrap_err().code,
            "source_changed"
        );
        root.unlink("a/b/c", Some(&stamp_json(&s))).unwrap();
        assert!(root.stat("a/b/c").unwrap().is_none());
        assert_eq!(root.stat("a/b").unwrap_err().code, "unsafe_file");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn absent_files_are_none_and_folders_are_not_files() {
        let root = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(root.path().join("a/b")).unwrap();
        std::fs::write(root.path().join("a/b/c.json"), b"{}").unwrap();
        assert_eq!(
            read_bytes(root.path(), "a/b/c.json", 64).unwrap().unwrap(),
            b"{}"
        );
        assert!(read_bytes(root.path(), "a/x/c.json", 64).unwrap().is_none());
        assert!(
            read_bytes(root.path(), "a/b/c.json/d", 64)
                .unwrap()
                .is_none()
        );
        assert_eq!(
            read_bytes(root.path(), "a/b", 64).unwrap_err().code,
            "unsafe_file"
        );
        assert_eq!(
            read_bytes(root.path(), "a/b/c.json", 1).unwrap_err().code,
            "file_too_large"
        );
        assert_eq!(
            read_bytes(root.path(), "../x", 64).unwrap_err().code,
            "unsafe_path"
        );
        assert_eq!(file_size(root.path(), "a/b/c.json").unwrap(), Some(2));
    }
}
