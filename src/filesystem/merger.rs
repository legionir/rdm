//! Chunk assembly into the final output file with size/checksum verification.

use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use sha2::{Digest, Sha256};
use tracing::{debug, info};

pub struct MergeReport {
    pub bytes: u64,
    pub chunks: usize,
}

/// Stream-copy every chunk (in order) into `output`, then fsync and atomically
/// expose it via a temp-file rename. Proceeds off the async runtime for
/// predictable throughput on large files.
pub async fn merge_chunks(
    chunks: impl IntoIterator<Item = PathBuf>,
    output: &Path,
    expected_total: u64,
) -> Result<MergeReport> {
    let chunks: Vec<PathBuf> = chunks.into_iter().collect();
    let output = output.to_path_buf();
    let expected = expected_total;
    tokio::task::spawn_blocking(move || merge_sync(&chunks, &output, expected))
        .await
        .context("assembly task panicked")?
}

fn merge_sync(chunks: &[PathBuf], output: &Path, expected_total: u64) -> Result<MergeReport> {
    if let Some(parent) = output.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("cannot create {}", parent.display()))?;
    }

    let tmp = output.with_extension(format!(
        "{}.rdm-part",
        output.extension().and_then(|e| e.to_str()).unwrap_or("file")
    ));
    let _ = std::fs::remove_file(&tmp);

    let mut out = std::fs::File::create(&tmp)
        .with_context(|| format!("cannot create assembly file {}", tmp.display()))?;
    let mut total: u64 = 0;
    let mut writer_buf = [0u8; 1024 * 1024];

    for (i, path) in chunks.iter().enumerate() {
        let meta = std::fs::metadata(path)
            .with_context(|| format!("chunk {} is missing ({})", i + 1, path.display()))?;
        let mut file = std::fs::File::open(path)
            .with_context(|| format!("cannot open chunk {}", path.display()))?;
        let mut remaining = meta.len();
        out.seek(SeekFrom::Start(total))?;
        while remaining > 0 {
            let n = remaining.min(writer_buf.len() as u64) as usize;
            let read = file
                .read(&mut writer_buf[..n])
                .context("reading chunk failed")?;
            if read == 0 {
                bail!("chunk {} truncated on disk ({})", i + 1, path.display());
            }
            out.write_all(&writer_buf[..read])
                .context("writing assembled file failed (disk full?)")?;
            total += read as u64;
            remaining -= read as u64;
        }
        debug!(chunk = i + 1, bytes = meta.len(), "merged chunk");
    }

    if expected_total > 0 && total != expected_total {
        let sizes: Vec<(String, u64)> = chunks
            .iter()
            .map(|p| {
                (
                    p.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_default(),
                    std::fs::metadata(p).map(|m| m.len()).unwrap_or(u64::MAX),
                )
            })
            .collect();
        let _ = std::fs::remove_file(&tmp);
        bail!(
            "assembly size mismatch: expected {expected_total} bytes, got {total} (chunk sizes {sizes:?})"
        );
    }

    out.set_len(total)?;
    out.sync_all().context("fsync failed")?;
    drop(out);
    std::fs::rename(&tmp, output).with_context(|| {
        format!("cannot move assembled file into place at {}", output.display())
    })?;
    // Best-effort directory durability on unix.
    #[cfg(unix)]
    if let Some(parent) = output.parent() {
        if let Ok(dir) = std::fs::File::open(parent) {
            let _ = dir.sync_all();
        }
    }

    info!(total, chunks = chunks.len(), "assembly finished");
    Ok(MergeReport {
        bytes: total,
        chunks: chunks.len(),
    })
}

/// Compute the SHA-256 of a file (streamed, bounded memory).
pub async fn compute_sha256(path: &Path) -> Result<String> {
    let path = path.to_path_buf();
    tokio::task::spawn_blocking(move || {
        let mut file = std::fs::File::open(&path)
            .with_context(|| format!("cannot open {} for checksum", path.display()))?;
        let mut hasher = Sha256::new();
        let mut buf = vec![0u8; 1024 * 1024];
        loop {
            let n = file.read(&mut buf)?;
            if n == 0 {
                break;
            }
            hasher.update(&buf[..n]);
        }
        let digest = hasher.finalize();
        Ok(hex::encode(digest))
    })
    .await
    .context("checksum task panicked")?
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[tokio::test]
    async fn merges_and_verifies() {
        let dir = std::env::temp_dir().join(format!("rdm-merger-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let chunks = vec![
            dir.join("c1.tmp"),
            dir.join("c2.tmp"),
            dir.join("c3.tmp"),
        ];
        let payloads: [&[u8]; 3] = [b"hello ", b"world", b"!!"];
        for (i, p) in payloads.iter().enumerate() {
            fs::write(&chunks[i], p).unwrap();
        }
        let out = dir.join("out.bin");
        let total: u64 = payloads.iter().map(|p| p.len() as u64).sum();
        let report = merge_chunks(chunks, &out, total).await.unwrap();
        assert_eq!(report.bytes, 13);
        assert_eq!(fs::read(&out).unwrap(), b"hello world!!");
        let digest = compute_sha256(&out).await.unwrap();
        assert_eq!(digest.len(), 64);
        let _ = fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn rejects_size_mismatch() {
        let dir = std::env::temp_dir().join(format!("rdm-merger2-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let chunk = dir.join("c.tmp");
        fs::write(&chunk, b"abcd").unwrap();
        let out = dir.join("out.bin");
        assert!(merge_chunks(vec![chunk], &out, 999).await.is_err());
        let _ = fs::remove_dir_all(&dir);
    }
}
