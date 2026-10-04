use super::*;
use rand::RngCore;
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use tokio::sync::oneshot;

const HEADER_MAGIC: &[u8; 8] = b"RSAIDJ01";
const FRAME_MAGIC: &[u8; 8] = b"RSAIFR01";
const TRAILER_MAGIC: &[u8; 8] = b"RSAIEND1";
const FORMAT_VERSION: u32 = 1;
const HEADER_LEN: usize = 8 + 4 + 16 + 32;
const PREFIX_LEN: usize = 8 + 8 + 8 + 32 + 32;
const TRAILER_LEN: usize = 8 + 8 + 8 + 32;

fn owners() -> &'static Mutex<HashSet<PathBuf>> {
    static OWNERS: OnceLock<Mutex<HashSet<PathBuf>>> = OnceLock::new();
    OWNERS.get_or_init(|| Mutex::new(HashSet::new()))
}

struct PathOwner(PathBuf);
impl Drop for PathOwner {
    fn drop(&mut self) {
        owners().lock().unwrap().remove(&self.0);
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum JournalFaultPoint {
    Append,
    ShortWrite,
    Sync,
    Acknowledge,
}

struct Inner {
    file: Option<File>,
    owner: Option<PathOwner>,
    snapshot: StorageSnapshot,
    last_hash: [u8; 32],
    writer: Option<u64>,
    poisoned: bool,
    closed: bool,
    fault: Option<JournalFaultPoint>,
}

struct CommitBarrier {
    reached: oneshot::Sender<()>,
    release: oneshot::Receiver<()>,
}

pub struct JournalStorage {
    path: PathBuf,
    inner: Mutex<Inner>,
    commit_barrier: Mutex<Option<CommitBarrier>>,
}

impl JournalStorage {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, DurableError> {
        let path = absolute(path.as_ref())?;
        {
            let mut owned = owners().lock().unwrap();
            if !owned.insert(path.clone()) {
                return Err(DurableError::Rejected(
                    "journal path already open in this process".into(),
                ));
            }
        }
        let owner = PathOwner(path.clone());
        let existed = path.exists();
        if let Some(parent) = path.parent() {
            create_private_directories(parent)?;
        }
        let mut options = OpenOptions::new();
        options.read(true).write(true).append(true);
        if !existed {
            options.create_new(true);
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options.open(&path).map_err(io)?;
        let metadata = file.metadata().map_err(io)?;
        if !metadata.is_file() {
            return Err(DurableError::Rejected(
                "journal path is not a regular file".into(),
            ));
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if metadata.permissions().mode() & 0o077 != 0 {
                return Err(DurableError::Rejected(
                    "journal file must not be group/world accessible".into(),
                ));
            }
        }
        let len = metadata.len();
        if !existed {
            write_header(&mut file)?;
            file.sync_all().map_err(uncertain)?;
            sync_parent(&path)?;
        } else if len == 0 {
            return Err(DurableError::Corrupt("existing journal is empty".into()));
        }
        let (snapshot, last_hash, valid_len) = scan(&mut file)?;
        let len = file.metadata().map_err(io)?.len();
        if valid_len < len {
            file.set_len(valid_len).map_err(io)?;
            file.sync_all().map_err(uncertain)?;
        }
        file.seek(SeekFrom::End(0)).map_err(io)?;
        Ok(Self {
            path,
            inner: Mutex::new(Inner {
                file: Some(file),
                owner: Some(owner),
                snapshot,
                last_hash,
                writer: None,
                poisoned: false,
                closed: false,
                fault: None,
            }),
            commit_barrier: Mutex::new(None),
        })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    #[doc(hidden)]
    pub fn inject_fault(&self, point: JournalFaultPoint) {
        self.inner.lock().unwrap().fault = Some(point);
    }

    #[doc(hidden)]
    pub fn inject_commit_barrier(&self) -> (oneshot::Receiver<()>, oneshot::Sender<()>) {
        let (reached_tx, reached_rx) = oneshot::channel();
        let (release_tx, release_rx) = oneshot::channel();
        *self.commit_barrier.lock().unwrap() = Some(CommitBarrier {
            reached: reached_tx,
            release: release_rx,
        });
        (reached_rx, release_tx)
    }

    fn check(inner: &Inner, claim: &WriterClaim, allow_poisoned: bool) -> Result<(), DurableError> {
        if inner.closed || inner.writer != Some(claim.id()) {
            return Err(DurableError::Closed);
        }
        if inner.poisoned && !allow_poisoned {
            return Err(DurableError::Poisoned);
        }
        Ok(())
    }
}

impl DurableStorage for JournalStorage {
    fn claim_writer(&self) -> Result<WriterClaim, DurableError> {
        let mut inner = self.inner.lock().unwrap();
        if inner.closed || inner.writer.is_some() {
            return Err(DurableError::Rejected(
                "storage writer already claimed".into(),
            ));
        }
        let claim = WriterClaim::fresh();
        inner.writer = Some(claim.id());
        Ok(claim)
    }

    fn load<'a>(&'a self, claim: &'a WriterClaim) -> StorageFuture<'a, StorageSnapshot> {
        Box::pin(async move {
            let inner = self.inner.lock().unwrap();
            Self::check(&inner, claim, false)?;
            Ok(inner.snapshot.clone())
        })
    }

    fn commit<'a>(&'a self, claim: &'a WriterClaim, batch: CommitBatch) -> StorageFuture<'a, ()> {
        Box::pin(async move {
            let barrier = self.commit_barrier.lock().unwrap().take();
            if let Some(barrier) = barrier {
                let _ = barrier.reached.send(());
                let _ = barrier.release.await;
            }
            let mut inner = self.inner.lock().unwrap();
            Self::check(&inner, claim, false)?;
            let payload = validate_batch(&inner.snapshot, &batch)?;
            let frame = encode_frame(batch.seq, inner.last_hash, &payload)?;
            let fault = inner.fault.take();
            if fault == Some(JournalFaultPoint::Append) {
                inner.poisoned = true;
                return Err(DurableError::Uncertain("injected append failure".into()));
            }
            let file = inner.file.as_mut().ok_or(DurableError::Closed)?;
            if fault == Some(JournalFaultPoint::ShortWrite) {
                let cut = (frame.len() / 2).max(1);
                if let Err(error) = file.write_all(&frame[..cut]) {
                    inner.poisoned = true;
                    return Err(uncertain(error));
                }
                inner.poisoned = true;
                return Err(DurableError::Uncertain("injected short write".into()));
            }
            if let Err(error) = file.write_all(&frame) {
                inner.poisoned = true;
                return Err(uncertain(error));
            }
            if fault == Some(JournalFaultPoint::Sync) {
                inner.poisoned = true;
                return Err(DurableError::Uncertain("injected sync failure".into()));
            }
            if let Err(error) = file.sync_all() {
                inner.poisoned = true;
                return Err(uncertain(error));
            }
            if fault == Some(JournalFaultPoint::Acknowledge) {
                inner.poisoned = true;
                return Err(DurableError::Uncertain(
                    "injected acknowledgement loss".into(),
                ));
            }
            let mut staged = inner.snapshot.clone();
            staged.apply(&batch)?;
            inner.snapshot = staged;
            inner.last_hash = Sha256::digest(&frame).into();
            Ok(())
        })
    }

    fn close<'a>(&'a self, claim: &'a WriterClaim) -> StorageFuture<'a, ()> {
        Box::pin(async move {
            let mut inner = self.inner.lock().unwrap();
            Self::check(&inner, claim, true)?;
            inner
                .file
                .as_ref()
                .ok_or(DurableError::Closed)?
                .sync_all()
                .map_err(uncertain)?;
            inner.closed = true;
            inner.writer = None;
            inner.file.take();
            inner.owner.take();
            Ok(())
        })
    }
}

fn absolute(path: &Path) -> Result<PathBuf, DurableError> {
    if path.exists() {
        return std::fs::canonicalize(path).map_err(io);
    }
    let name = path
        .file_name()
        .ok_or_else(|| DurableError::Io("journal path has no file name".into()))?;
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let absolute_parent = if parent.is_absolute() {
        parent.to_path_buf()
    } else {
        std::env::current_dir().map_err(io)?.join(parent)
    };
    create_private_directories(&absolute_parent)?;
    Ok(std::fs::canonicalize(&absolute_parent)
        .map_err(io)?
        .join(name))
}

fn create_private_directories(path: &Path) -> Result<(), DurableError> {
    if path.exists() {
        return Ok(());
    }
    let parent = path
        .parent()
        .ok_or_else(|| DurableError::Io("directory has no parent".into()))?;
    create_private_directories(parent)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
        let mut builder = std::fs::DirBuilder::new();
        builder.mode(0o700);
        builder.create(path).map_err(io)?;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700)).map_err(io)?;
    }
    #[cfg(not(unix))]
    std::fs::create_dir(path).map_err(io)?;
    File::open(parent)
        .map_err(io)?
        .sync_all()
        .map_err(uncertain)?;
    Ok(())
}

fn sync_parent(path: &Path) -> Result<(), DurableError> {
    if let Some(parent) = path.parent() {
        File::open(parent)
            .map_err(io)?
            .sync_all()
            .map_err(uncertain)?;
    }
    Ok(())
}

fn write_header(file: &mut File) -> Result<(), DurableError> {
    let mut bytes = Vec::with_capacity(HEADER_LEN);
    bytes.extend_from_slice(HEADER_MAGIC);
    bytes.extend_from_slice(&FORMAT_VERSION.to_le_bytes());
    let mut session_id = [0u8; 16];
    rand::rngs::OsRng.fill_bytes(&mut session_id);
    bytes.extend_from_slice(&session_id);
    let digest: [u8; 32] = Sha256::digest(&bytes).into();
    bytes.extend_from_slice(&digest);
    file.write_all(&bytes).map_err(io)
}

fn encode_frame(
    seq: CommitSeq,
    previous: [u8; 32],
    payload: &[u8],
) -> Result<Vec<u8>, DurableError> {
    let len =
        u64::try_from(payload.len()).map_err(|_| DurableError::Range("payload length".into()))?;
    let digest: [u8; 32] = Sha256::digest(payload).into();
    let mut out = Vec::with_capacity(PREFIX_LEN + payload.len() + TRAILER_LEN);
    out.extend_from_slice(FRAME_MAGIC);
    out.extend_from_slice(&seq.get().to_le_bytes());
    out.extend_from_slice(&len.to_le_bytes());
    out.extend_from_slice(&previous);
    out.extend_from_slice(&digest);
    out.extend_from_slice(payload);
    out.extend_from_slice(TRAILER_MAGIC);
    out.extend_from_slice(&seq.get().to_le_bytes());
    out.extend_from_slice(&len.to_le_bytes());
    out.extend_from_slice(&digest);
    Ok(out)
}

fn scan(file: &mut File) -> Result<(StorageSnapshot, [u8; 32], u64), DurableError> {
    file.seek(SeekFrom::Start(0)).map_err(io)?;
    let mut header = vec![0u8; HEADER_LEN];
    file.read_exact(&mut header)
        .map_err(|error| DurableError::Corrupt(format!("header: {error}")))?;
    if &header[..8] != HEADER_MAGIC {
        return Err(DurableError::Corrupt("bad header magic".into()));
    }
    if u32::from_le_bytes(header[8..12].try_into().unwrap()) != FORMAT_VERSION {
        return Err(DurableError::Corrupt("unsupported journal version".into()));
    }
    let expected: [u8; 32] = Sha256::digest(&header[..28]).into();
    if header[28..] != expected {
        return Err(DurableError::Corrupt("bad header checksum".into()));
    }

    let mut snapshot = StorageSnapshot::empty();
    let mut previous = [0u8; 32];
    let mut offset = HEADER_LEN as u64;
    let total = file.metadata().map_err(io)?.len();
    while offset < total {
        let remaining = total - offset;
        file.seek(SeekFrom::Start(offset)).map_err(io)?;
        let probe_len = remaining.min(PREFIX_LEN as u64) as usize;
        let mut probe = vec![0u8; probe_len];
        file.read_exact(&mut probe).map_err(io)?;
        check_available("frame magic", &probe, 0, FRAME_MAGIC)?;
        check_available(
            "frame sequence",
            &probe,
            8,
            &snapshot.next_seq.to_le_bytes(),
        )?;
        if probe.len() >= 24 {
            let len = u64::from_le_bytes(probe[16..24].try_into().unwrap());
            if len > MAX_COMMIT_BYTES as u64 {
                return Err(DurableError::Corrupt("frame length exceeds limit".into()));
            }
        } else if probe.len() > 16 {
            let available = probe.len() - 16;
            let partial = &probe[16..];
            let min_value = u64::from_le_bytes({
                let mut bytes = [0u8; 8];
                bytes[..available].copy_from_slice(partial);
                bytes
            });
            if min_value > MAX_COMMIT_BYTES as u64 {
                return Err(DurableError::Corrupt("invalid partial frame length".into()));
            }
        }
        check_available("previous frame digest", &probe, 24, &previous)?;
        if remaining < PREFIX_LEN as u64 {
            return Ok((snapshot, previous, offset));
        }
        let prefix = probe;
        let seq = u64::from_le_bytes(prefix[8..16].try_into().unwrap());
        let len = u64::from_le_bytes(prefix[16..24].try_into().unwrap());
        let payload_end = PREFIX_LEN as u64 + len;
        let full = payload_end + TRAILER_LEN as u64;
        if remaining < full {
            if remaining > payload_end {
                file.seek(SeekFrom::Start(offset + PREFIX_LEN as u64))
                    .map_err(io)?;
                let mut payload = vec![0u8; len as usize];
                file.read_exact(&mut payload).map_err(io)?;
                let digest: [u8; 32] = Sha256::digest(&payload).into();
                if prefix[56..88] != digest {
                    return Err(DurableError::Corrupt("payload checksum mismatch".into()));
                }
                let trailer_len = (remaining - payload_end) as usize;
                let mut trailer = vec![0u8; trailer_len];
                file.read_exact(&mut trailer).map_err(io)?;
                check_available("trailer magic", &trailer, 0, TRAILER_MAGIC)?;
                check_available("trailer sequence", &trailer, 8, &seq.to_le_bytes())?;
                check_available("trailer length", &trailer, 16, &len.to_le_bytes())?;
                check_available("trailer digest", &trailer, 24, &digest)?;
            }
            return Ok((snapshot, previous, offset));
        }
        let mut payload = vec![0u8; len as usize];
        file.read_exact(&mut payload).map_err(io)?;
        let digest: [u8; 32] = Sha256::digest(&payload).into();
        if prefix[56..88] != digest {
            return Err(DurableError::Corrupt("payload checksum mismatch".into()));
        }
        let mut trailer = vec![0u8; TRAILER_LEN];
        file.read_exact(&mut trailer).map_err(io)?;
        if &trailer[..8] != TRAILER_MAGIC
            || trailer[8..16] != seq.to_le_bytes()
            || trailer[16..24] != len.to_le_bytes()
            || trailer[24..56] != digest
        {
            return Err(DurableError::Corrupt("bad frame trailer".into()));
        }
        let batch: CommitBatch = serde_json::from_slice(&payload)
            .map_err(|error| DurableError::Corrupt(error.to_string()))?;
        if batch.seq.get() != seq {
            return Err(DurableError::Corrupt(
                "frame/payload sequence mismatch".into(),
            ));
        }
        snapshot
            .apply(&batch)
            .map_err(|error| DurableError::Corrupt(error.to_string()))?;
        file.seek(SeekFrom::Start(offset)).map_err(io)?;
        let mut frame = vec![0u8; full as usize];
        file.read_exact(&mut frame).map_err(io)?;
        previous = Sha256::digest(&frame).into();
        offset += full;
    }
    Ok((snapshot, previous, offset))
}

fn check_available(
    field: &str,
    available: &[u8],
    offset: usize,
    expected: &[u8],
) -> Result<(), DurableError> {
    if available.len() <= offset {
        return Ok(());
    }
    let count = (available.len() - offset).min(expected.len());
    if available[offset..offset + count] != expected[..count] {
        return Err(DurableError::Corrupt(format!("invalid partial {field}")));
    }
    Ok(())
}

fn io(error: std::io::Error) -> DurableError {
    DurableError::Io(error.to_string())
}
fn uncertain(error: std::io::Error) -> DurableError {
    DurableError::Uncertain(error.to_string())
}
