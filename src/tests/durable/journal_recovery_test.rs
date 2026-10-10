#[cfg(test)]
mod tests {
    use crate::durable::storage::journal::JournalFaultPoint;
    use crate::durable::*;
    use serde_json::json;
    use std::io::Write;

    fn batch() -> CommitBatch {
        let seq = CommitSeq::new(1).unwrap();
        let conversation = ConversationId::new(1).unwrap();
        CommitBatch {
            conversations: vec![],
            seq,
            next_id: 3,
            next_seq: 2,
            entries: vec![EntryRecord {
                id: EntryId::new(2).unwrap(),
                conversation_id: conversation,
                kind: "user".into(),
                value: json!({"x":1}),
                by_task_id: None,
                created_seq: seq,
            }],
            tasks: vec![],
            submissions: vec![],
            documents: vec![],
        }
    }
    fn path(label: &str) -> std::path::PathBuf {
        let p = std::env::temp_dir().join(format!("rs-ai-durable-{label}-{}", std::process::id()));
        let _ = std::fs::remove_file(&p);
        p
    }

    fn write_private(path: &std::path::Path, bytes: &[u8]) {
        use std::fs::OpenOptions;
        #[cfg(unix)]
        use std::os::unix::fs::OpenOptionsExt;
        let mut options = OpenOptions::new();
        options.create_new(true).write(true);
        #[cfg(unix)]
        options.mode(0o600);
        options.open(path).unwrap().write_all(bytes).unwrap();
    }

    #[tokio::test]
    async fn acknowledged_commit_survives_reopen_and_partial_final_tail_is_truncated() {
        let p = path("reopen");
        {
            let s = JournalStorage::open(&p).unwrap();
            let claim = s.claim_writer().unwrap();
            s.commit(&claim, batch()).await.unwrap();
            s.close(&claim).await.unwrap();
        }
        let valid_len = std::fs::metadata(&p).unwrap().len();
        {
            let mut f = std::fs::OpenOptions::new().append(true).open(&p).unwrap();
            f.write_all(b"RSAIFR01").unwrap();
            f.write_all(&2u64.to_le_bytes()).unwrap();
            f.sync_all().unwrap();
        }
        let s = JournalStorage::open(&p).unwrap();
        let claim = s.claim_writer().unwrap();
        assert_eq!(s.load(&claim).await.unwrap().entries.len(), 1);
        assert_eq!(std::fs::metadata(&p).unwrap().len(), valid_len);
        let _ = std::fs::remove_file(p);
    }

    #[tokio::test]
    async fn arbitrary_junk_and_corrupt_complete_prefix_fail_closed() {
        let p = path("junk");
        {
            let storage = JournalStorage::open(&p).unwrap();
            let claim = storage.claim_writer().unwrap();
            storage.close(&claim).await.unwrap();
        }
        let base = std::fs::read(&p).unwrap();
        for tail in [b"JUNK".as_slice(), b"RSAIFR01\x09\0\0\0\0\0\0\0".as_slice()] {
            let candidate = path("bad-prefix");
            let mut bytes = base.clone();
            bytes.extend_from_slice(tail);
            write_private(&candidate, &bytes);
            assert!(matches!(
                JournalStorage::open(&candidate),
                Err(DurableError::Corrupt(_))
            ));
            let _ = std::fs::remove_file(candidate);
        }
        let _ = std::fs::remove_file(p);
    }

    #[tokio::test]
    async fn invalid_partial_sequence_chain_and_trailer_fail_closed() {
        let base = path("partial-prefix-base");
        {
            let storage = JournalStorage::open(&base).unwrap();
            let claim = storage.claim_writer().unwrap();
            storage.commit(&claim, batch()).await.unwrap();
            storage.close(&claim).await.unwrap();
        }
        let bytes = std::fs::read(&base).unwrap();
        let header = 60usize;
        let payload_len =
            u64::from_le_bytes(bytes[header + 16..header + 24].try_into().unwrap()) as usize;
        let trailer = header + 88 + payload_len;
        for (label, cut, index) in [
            ("seq", header + 9, header + 8),
            ("chain", header + 25, header + 24),
            ("trailer", trailer + 1, trailer),
        ] {
            let p = path(&format!("partial-{label}"));
            let mut changed = bytes[..cut].to_vec();
            changed[index] ^= 0xff;
            write_private(&p, &changed);
            assert!(
                matches!(JournalStorage::open(&p), Err(DurableError::Corrupt(_))),
                "{label}"
            );
            let _ = std::fs::remove_file(p);
        }
        let _ = std::fs::remove_file(base);
    }

    #[tokio::test]
    async fn every_final_frame_byte_cut_reopens_to_prior_truth() {
        let base = path("cuts-base");
        let header_len;
        {
            let s = JournalStorage::open(&base).unwrap();
            let claim = s.claim_writer().unwrap();
            header_len = std::fs::metadata(&base).unwrap().len();
            s.commit(&claim, batch()).await.unwrap();
            s.close(&claim).await.unwrap();
        }
        let bytes = std::fs::read(&base).unwrap();
        for cut in header_len as usize..bytes.len() {
            let p = path(&format!("cut-{cut}"));
            write_private(&p, &bytes[..cut]);
            let s = JournalStorage::open(&p).unwrap();
            let claim = s.claim_writer().unwrap();
            assert!(
                s.load(&claim).await.unwrap().entries.is_empty(),
                "cut {cut}"
            );
            let _ = std::fs::remove_file(p);
        }
        let _ = std::fs::remove_file(base);
    }

    #[tokio::test]
    async fn fully_present_checksum_trailer_and_interior_corruption_fail_closed() {
        let base = path("corrupt");
        {
            let s = JournalStorage::open(&base).unwrap();
            let claim = s.claim_writer().unwrap();
            s.commit(&claim, batch()).await.unwrap();
            s.close(&claim).await.unwrap();
        }
        let bytes = std::fs::read(&base).unwrap();
        for index in [20usize, 60, bytes.len() - 1] {
            let p = path(&format!("flip-{index}"));
            let mut changed = bytes.clone();
            changed[index] ^= 0x55;
            write_private(&p, &changed);
            assert!(
                matches!(JournalStorage::open(&p), Err(DurableError::Corrupt(_))),
                "corruption at byte {index} was not rejected"
            );
            let _ = std::fs::remove_file(p);
        }
        let _ = std::fs::remove_file(base);
    }

    #[tokio::test]
    async fn actual_append_sync_and_ack_faults_poison_until_reopen() {
        for point in [
            JournalFaultPoint::Append,
            JournalFaultPoint::ShortWrite,
            JournalFaultPoint::Sync,
            JournalFaultPoint::Acknowledge,
        ] {
            let p = path(&format!("fault-{point:?}"));
            {
                let storage = JournalStorage::open(&p).unwrap();
                let claim = storage.claim_writer().unwrap();
                storage.inject_fault(point);
                assert!(matches!(
                    storage.commit(&claim, batch()).await,
                    Err(DurableError::Uncertain(_))
                ));
                assert!(matches!(
                    storage.load(&claim).await,
                    Err(DurableError::Poisoned)
                ));
                assert!(matches!(
                    storage.commit(&claim, batch()).await,
                    Err(DurableError::Poisoned)
                ));
                storage.close(&claim).await.unwrap();
            }
            let storage = JournalStorage::open(&p).unwrap();
            let claim = storage.claim_writer().unwrap();
            let committed = storage.load(&claim).await.unwrap().entries.len();
            assert_eq!(
                committed,
                usize::from(matches!(
                    point,
                    JournalFaultPoint::Sync | JournalFaultPoint::Acknowledge
                ))
            );
            storage.close(&claim).await.unwrap();
            let _ = std::fs::remove_file(p);
        }
    }

    #[test]
    fn existing_empty_file_and_second_live_handle_fail_closed() {
        let empty = path("empty");
        write_private(&empty, &[]);
        assert!(matches!(
            JournalStorage::open(&empty),
            Err(DurableError::Corrupt(_))
        ));
        let _ = std::fs::remove_file(empty);

        let p = path("owner");
        let first = JournalStorage::open(&p).unwrap();
        assert!(matches!(
            JournalStorage::open(&p),
            Err(DurableError::Rejected(_))
        ));
        drop(first);
        assert!(JournalStorage::open(&p).is_ok());
        let _ = std::fs::remove_file(p);
    }

    #[tokio::test]
    async fn session_reopen_observes_durable_truth() {
        let p = path("session-reopen");
        {
            let s = DurableSession::open(Box::new(JournalStorage::open(&p).unwrap()))
                .await
                .unwrap();
            s.commit(batch()).await.unwrap();
            s.close().await.unwrap();
        }
        let s = DurableSession::open(Box::new(JournalStorage::open(&p).unwrap()))
            .await
            .unwrap();
        assert_eq!(s.snapshot().await.unwrap().entries.len(), 1);
        s.close().await.unwrap();
        let _ = std::fs::remove_file(p);
    }
}
