use super::*;
use std::io::{Read, Seek, SeekFrom};
use std::time::{Duration, Instant};

#[test]
fn writes_complete_file_and_replaces_it() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("new/state");
    atomic_write(&path, b"first").unwrap();
    atomic_write_with(&path, b"second", SyncMode::Plain).unwrap();
    assert_eq!(fs::read(&path).unwrap(), b"second");
    assert_eq!(fs::read_dir(parent(&path)).unwrap().count(), 1);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );
    }
}

#[test]
fn failed_rename_cleans_up_temporary() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("occupied");
    fs::create_dir(&path).unwrap();
    assert!(matches!(
        atomic_write(&path, b"new"),
        Err(AtomicWriteError::BeforeRename(_))
    ));
    assert!(path.is_dir());
    assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 1);
}

#[test]
fn directory_sync_failure_is_reported_after_the_new_file_is_visible() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("state");
    atomic_write(&path, b"old").unwrap();
    let result = atomic_write_using(&path, b"new", SyncMode::Plain, |parent, mode| {
        assert_eq!(parent, dir.path());
        assert_eq!(mode, SyncMode::Plain);
        assert_eq!(fs::read(&path).unwrap(), b"new");
        Err(io::Error::from(io::ErrorKind::PermissionDenied))
    });
    assert!(matches!(result, Err(AtomicWriteError::AfterRename(error))
        if error.kind() == io::ErrorKind::PermissionDenied));
    assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 1);
}

#[test]
fn exclusive_publish_preserves_an_existing_target() {
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("source");
    let target = dir.path().join("target");
    atomic_write(&source, b"source").unwrap();
    atomic_write(&target, b"target").unwrap();
    assert_eq!(
        publish_new_locked(&source, &target, SyncMode::Full)
            .unwrap_err()
            .kind(),
        io::ErrorKind::AlreadyExists
    );
    assert_eq!(fs::read(&target).unwrap(), b"target");
    fs::remove_file(&target).unwrap();
    let placement = publish_new_locked(&source, &target, SyncMode::Full).unwrap();
    assert_eq!(fs::read(&target).unwrap(), b"source");
    assert_eq!(source.exists(), placement == Placement::Linked);
}

#[test]
fn shared_exclusive_deadline_and_probe() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("lock");
    assert!(!lock_held(&path).unwrap());
    assert!(!path.exists());
    fs::write(&path, b"").unwrap();
    let open = || File::options().read(true).write(true).open(&path).unwrap();
    let (first, second, third) = (open(), open(), open());
    lock(&first, LockMode::Shared, Duration::ZERO).unwrap();
    lock(&second, LockMode::Shared, Duration::ZERO).unwrap();
    assert!(lock_held(&path).unwrap());
    let wait = Duration::from_millis(20);
    let started = Instant::now();
    assert_eq!(
        lock(&third, LockMode::Exclusive, wait).unwrap_err().kind(),
        io::ErrorKind::WouldBlock
    );
    assert!(started.elapsed() >= wait);
    drop((first, second));
    lock(&third, LockMode::Exclusive, Duration::ZERO).unwrap();
    assert_eq!(
        lock(&open(), LockMode::Shared, Duration::ZERO)
            .unwrap_err()
            .kind(),
        io::ErrorKind::WouldBlock
    );
    drop(third);
    assert!(!lock_held(&path).unwrap());
}

#[test]
fn verification_reads_across_sector_and_block_boundaries() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("verified");
    for size in [0_usize, 1, 4095, 4096, 4097, 4 * 1024 * 1024 + 37] {
        let data: Vec<u8> = (0..size).map(|index| (index % 251) as u8).collect();
        fs::write(&path, &data).unwrap();
        let mut file = verify_read(File::open(&path).unwrap());
        let mut read = Vec::new();
        file.read_to_end(&mut read).unwrap();
        assert_eq!(read, data);
        file.seek(SeekFrom::Start(0)).unwrap();
        read.clear();
        file.read_to_end(&mut read).unwrap();
        assert_eq!(read, data);
        assert!(file.seek(SeekFrom::Start(1)).is_err());
    }
}
