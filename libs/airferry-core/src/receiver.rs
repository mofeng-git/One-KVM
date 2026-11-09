use anyhow::{bail, Context, Result};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    collections::VecDeque,
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    time::Instant,
};
use transfer_engine::{FileMeta, ReceiverSession, TransferAssembler};

pub struct Receiver {
    session: Option<ReceiverSession>,
    assembler: Option<TransferAssembler>,
    completed: VecDeque<u128>,
    output: PathBuf,
    max_size: u64,
    started: Option<Instant>,
    newest_sequence: u64,
    rates: VecDeque<(Instant, u32)>,
}
impl Receiver {
    pub fn new(output: &Path, max_size: u64) -> Result<Self> {
        fs::create_dir_all(output)?;
        Ok(Self {
            session: None,
            assembler: None,
            completed: VecDeque::new(),
            output: output.into(),
            max_size,
            started: None,
            newest_sequence: 0,
            rates: VecDeque::new(),
        })
    }
    pub fn progress(&self) -> Value {
        self.session
            .as_ref()
            .map(|s| {
                let p = s.progress();
                json!({"file":s.file_meta().filename,
            "received_symbols":p.received_symbols,"total_symbols":p.total_symbols,
            "symbol_size":p.symbol_size, "bytes_per_second":self.rate(p.received_symbols, p.symbol_size),
            "decoded_fraction":p.decoded_fraction(), "meta_confirmed":p.meta_confirmed})
            })
            .unwrap_or(Value::Null)
    }
    #[cfg(test)]
    pub fn ingest(&mut self, bytes: &[u8]) -> Result<Option<Value>> {
        self.ingest_sequenced(bytes, self.newest_sequence + 1)
    }
    // Parallel QR results may arrive out of order. Never switch back to an old
    // file because one of its slower frames finished after the new file began.
    pub fn ingest_sequenced(&mut self, bytes: &[u8], sequence: u64) -> Result<Option<Value>> {
        let frame = match transfer_engine::qr_protocol_frame_from_bytes(bytes) {
            Ok(f) => f,
            Err(_) => return Ok(None),
        };
        let id = frame.header.session_id;
        if self.completed.contains(&id) {
            return Ok(None);
        }
        if frame.header.flags & transfer_engine::FLAG_DESCRIPTOR_BIT != 0 {
            let desc = transfer_engine::descriptor::parse_payload(&frame.payload)
                .context("Invalid descriptor")?;
            if desc.file_meta.original_size > self.max_size
                || desc.file_meta.compressed_size > 32 * 1024 * 1024
            {
                bail!("Transfer exceeds configured size budget");
            }
            if desc
                .segment
                .as_ref()
                .is_some_and(|s| s.root_original_size > self.max_size)
            {
                bail!("Segmented transfer exceeds compressed size budget");
            }
        }
        if self.session.as_ref().is_none_or(|s| s.session_id() != id) {
            if sequence < self.newest_sequence {
                return Ok(None);
            }
            if self.assembler.is_none() {
                self.started = Some(Instant::now());
            }
            self.rates.clear();
            self.rates.push_back((Instant::now(), 0));
            self.session = Some(ReceiverSession::from_first_frame(&frame));
        }
        self.newest_sequence = self.newest_sequence.max(sequence);
        let session = self.session.as_mut().unwrap();
        session.ingest(frame)?;
        let now = Instant::now();
        self.rates
            .push_back((now, session.progress().received_symbols));
        while self.rates.len() > 1
            && (now.duration_since(self.rates[0].0).as_secs_f64() > 3.0 || self.rates.len() > 4096)
        {
            self.rates.pop_front();
        }
        if !session.is_complete() || !session.is_meta_confirmed() {
            return Ok(None);
        }
        let meta = session.file_meta().clone();
        let (data, task_id) = if let Some(segment) = session.segment_meta().cloned() {
            let raw = session.assemble_raw().context("Missing segment payload")?;
            if self
                .assembler
                .as_ref()
                .is_none_or(|a| a.root_session_id() != segment.root_session_id)
            {
                self.assembler = Some(TransferAssembler::new(&segment, &meta)?);
            }
            let assembler = self.assembler.as_mut().unwrap();
            assembler.add_segment(&segment, &meta, raw)?;
            let progress = json!({"state":"segment_saved", "received":assembler.received_segments(),
                "total":assembler.total_segments()});
            if !assembler.is_complete() {
                self.completed.push_back(id);
                while self.completed.len() > 64 {
                    self.completed.pop_front();
                }
                self.session = None;
                return Ok(Some(progress));
            }
            let compressed = assembler
                .reassemble()
                .context("Incomplete segment assembly")?;
            let limit = usize::try_from(self.max_size)?;
            let data =
                qr_protocol::compress::decompress_with_limit(&compressed, meta.compression, limit)?;
            if Sha256::digest(&data).as_slice() != segment.root_sha256 {
                bail!("Root SHA256 mismatch");
            }
            (data, segment.root_session_id)
        } else {
            (
                session
                    .assemble_result()?
                    .context("Incomplete recovered payload")?,
                id,
            )
        };
        verify(&meta, &data, self.max_size)?;
        let filename = safe_filename(&meta.filename);
        let name = format!("{task_id:032x}-{filename}");
        let path = self.output.join(&name);
        // A stopped process may leave its partial file behind. A unique name
        // keeps that file from blocking a new reception of the same task.
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_nanos();
        let temp = self
            .output
            .join(format!(".airferry-{}-{nonce}.part", std::process::id()));
        let hash = format!("{:x}", Sha256::digest(&data));
        let mut out = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp)?;
        let saved = (|| -> Result<()> {
            out.write_all(&data)?;
            out.sync_all()?;
            match fs::hard_link(&temp, &path) {
                Ok(()) => (),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                    // A repeated optical transmission after a restart is harmless.
                    // Verify the existing result; never replace a different file.
                    if fs::metadata(&path)?.len() != data.len() as u64
                        || format!("{:x}", Sha256::digest(fs::read(&path)?)) != hash
                    {
                        bail!("A different file already exists at the destination");
                    }
                }
                Err(error) => return Err(error.into()),
            }
            Ok(())
        })();
        drop(out);
        let _ = fs::remove_file(&temp);
        saved?;
        self.completed.push_back(id);
        while self.completed.len() > 64 {
            self.completed.pop_front();
        }
        self.session = None;
        self.assembler = None;
        let receive_seconds = self
            .started
            .take()
            .map(|s| s.elapsed().as_secs_f64())
            .unwrap_or(0.0);
        Ok(Some(
            json!({"state":"saved", "path":path, "filename":filename,"bytes":data.len(),"sha256":hash,
                "receive_seconds":receive_seconds,"bytes_per_second":data.len() as f64 / receive_seconds.max(0.001)}),
        ))
    }

    fn rate(&self, received: u32, symbol_size: u32) -> f64 {
        // AirFerry ReceivePage: unique-symbol delta × symbol size / elapsed,
        // over a three-second sliding window. Repeated QR codes add no speed.
        let now = Instant::now();
        let Some(&(start, count)) = self
            .rates
            .iter()
            .find(|(t, _)| now.duration_since(*t).as_secs_f64() <= 3.0)
        else {
            return 0.0;
        };
        let elapsed = now.duration_since(start).as_secs_f64();
        if elapsed < 0.2 {
            return 0.0;
        }
        f64::from(received.saturating_sub(count)) * f64::from(symbol_size) / elapsed
    }
}

fn verify(meta: &FileMeta, data: &[u8], max: u64) -> Result<()> {
    if data.len() as u64 > max || data.len() as u64 != meta.original_size {
        bail!("Recovered size mismatch");
    }
    if !meta.crc32_known {
        bail!("Descriptor does not provide a file checksum");
    }
    if crc32fast::hash(data) != meta.crc32 {
        bail!("Recovered CRC32 mismatch");
    }
    Ok(())
}
fn safe_filename(name: &str) -> String {
    let basename = name.rsplit(['/', '\\']).next().unwrap_or("");
    // Reserve 32 hex digits and '-' within the filesystem's 255-byte component limit.
    let mut bytes = 0;
    let cleaned: String = basename
        .chars()
        .filter(|c| !c.is_control())
        .take_while(|c| {
            bytes += c.len_utf8();
            bytes <= 255 - 33
        })
        .collect();
    if cleaned.is_empty() || cleaned == "." || cleaned == ".." {
        "received.bin".into()
    } else {
        cleaned
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn paths_remain_in_output() {
        assert_eq!(safe_filename("../../a.txt"), "a.txt");
        assert_eq!(safe_filename("C:\\tmp\\x.bin"), "x.bin");
        assert_eq!(safe_filename(".."), "received.bin");
        assert_eq!(safe_filename(&"中".repeat(120)).len(), 222);
        assert_eq!(safe_filename(&"a".repeat(300)).len(), 222);
    }

    #[test]
    fn long_unicode_filename_can_be_saved() {
        use qr_protocol::session::SessionId;
        use transfer_engine::{SenderConfig, SenderSession};
        let directory =
            std::env::temp_dir().join(format!("airferry-unicode-{}", std::process::id()));
        let data = vec![42; 1024];
        let meta = FileMeta {
            filename: "中".repeat(80) + ".bin",
            original_size: data.len() as u64,
            compressed_size: data.len() as u64,
            compressed_size_known: true,
            crc32: crc32fast::hash(&data),
            crc32_known: true,
            ..Default::default()
        };
        let mut sender =
            SenderSession::new(&data, SessionId(43), SenderConfig::default(), meta).unwrap();
        let mut receiver = Receiver::new(&directory, 65536).unwrap();
        let saved = (0..100)
            .find_map(|_| {
                receiver
                    .ingest(&sender.next_frame().unwrap().to_bytes())
                    .unwrap()
            })
            .expect("Unicode filenames must fit both final and temporary file names");
        let path = Path::new(saved["path"].as_str().unwrap());
        assert_eq!(fs::read(path).unwrap(), data);
        assert_eq!(path.file_name().unwrap().as_encoded_bytes().len(), 255);
        assert_eq!(fs::read_dir(&directory).unwrap().count(), 1);
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn segmented_transfer_can_retry_after_a_save_failure() {
        use qr_protocol::session::SessionId;
        use transfer_engine::{SegmentMeta, SenderConfig, SenderSession};
        let directory =
            std::env::temp_dir().join(format!("airferry-save-retry-{}", std::process::id()));
        let data = vec![42; 1024];
        let task_id = 44;
        let meta = FileMeta {
            filename: "test.bin".into(),
            original_size: data.len() as u64,
            compressed_size: data.len() as u64,
            compressed_size_known: true,
            crc32: crc32fast::hash(&data),
            crc32_known: true,
            ..Default::default()
        };
        let segment = SegmentMeta {
            root_session_id: task_id,
            segment_index: 0,
            segment_count: 1,
            original_offset: 0,
            root_original_size: data.len() as u64,
            root_sha256: Sha256::digest(&data).into(),
            raw_sha256: Sha256::digest(&data).into(),
        };
        let mut sender = SenderSession::new_segment(
            &data,
            SessionId::derive_segment(task_id, 0),
            SenderConfig::default(),
            meta,
            segment,
        )
        .unwrap();
        let mut receiver = Receiver::new(&directory, 65536).unwrap();
        let path = directory.join(format!("{task_id:032x}-test.bin"));
        fs::write(&path, b"conflicting file").unwrap();
        let error = (0..100)
            .find_map(|_| {
                receiver
                    .ingest(&sender.next_frame().unwrap().to_bytes())
                    .err()
            })
            .expect("A conflicting file must prevent saving");
        assert!(error.to_string().contains("different file"));
        fs::remove_file(&path).unwrap();
        let saved = (0..100)
            .find_map(|_| {
                receiver
                    .ingest(&sender.next_frame().unwrap().to_bytes())
                    .unwrap()
            })
            .expect("The final segment must remain retryable until the file is saved");
        assert_eq!(saved["state"], "saved");
        assert_eq!(fs::read(path).unwrap(), data);
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn speed_counts_unique_symbols_and_expires_when_idle() {
        let directory = std::env::temp_dir().join(format!("airferry-rate-{}", std::process::id()));
        let mut receiver = Receiver::new(&directory, 1024).unwrap();
        receiver
            .rates
            .push_back((Instant::now() - std::time::Duration::from_secs(2), 10));
        let rate = receiver.rate(110, 1400);
        assert!((69_000.0..=70_001.0).contains(&rate));
        assert_eq!(receiver.rate(10, 1400), 0.0); // duplicate payloads
        receiver.rates[0].0 = Instant::now() - std::time::Duration::from_secs(4);
        assert_eq!(receiver.rate(110, 1400), 0.0);
        fs::remove_dir(directory).unwrap();
    }

    #[test]
    fn late_result_cannot_restore_a_previous_file_session() {
        use qr_protocol::session::SessionId;
        use transfer_engine::{SenderConfig, SenderSession};
        let directory = std::env::temp_dir().join(format!("airferry-order-{}", std::process::id()));
        let data = vec![1; 16384];
        let mut old = SenderSession::new(
            &data,
            SessionId(101),
            SenderConfig::default(),
            FileMeta::default(),
        )
        .unwrap();
        let mut new = SenderSession::new(
            &data,
            SessionId(102),
            SenderConfig::default(),
            FileMeta::default(),
        )
        .unwrap();
        let mut receiver = Receiver::new(&directory, 65536).unwrap();
        receiver
            .ingest_sequenced(&old.next_frame().unwrap().to_bytes(), 1)
            .unwrap();
        receiver
            .ingest_sequenced(&new.next_frame().unwrap().to_bytes(), 3)
            .unwrap();
        receiver
            .ingest_sequenced(&old.next_frame().unwrap().to_bytes(), 2)
            .unwrap();
        assert_eq!(receiver.session.as_ref().unwrap().session_id(), 102);
        fs::remove_dir(directory).unwrap();
    }

    #[test]
    fn file_recovery_after_loss_and_duplicates_for_all_compression_types() {
        use qr_protocol::{compress, session::SessionId};
        use transfer_engine::{SenderConfig, SenderSession};
        for tag in 0..=2u8 {
            let directory =
                std::env::temp_dir().join(format!("airferry-test-{}-{tag}", std::process::id()));
            fs::create_dir(&directory).unwrap();
            let original: Vec<u8> = (0..8192).map(|i| (i * 7 + i / 251) as u8).collect();
            let payload = compress::compress_with(&original, tag).unwrap();
            let meta = FileMeta {
                filename: "../../test.bin".into(),
                original_size: original.len() as u64,
                crc32: crc32fast::hash(&original),
                compression: tag,
                compressed_size: payload.len() as u64,
                compressed_size_known: true,
                crc32_known: true,
            };
            let mut sender = SenderSession::new(
                &payload,
                SessionId(12 + u128::from(tag)),
                SenderConfig::default(),
                meta,
            )
            .unwrap();
            let mut receiver = Receiver::new(&directory, 1024 * 1024).unwrap();
            let mut saved = None;
            for index in 0..500 {
                let frame = sender.next_frame().unwrap().to_bytes();
                if index % 3 == 1 {
                    continue;
                }
                for _ in 0..2 {
                    if let Some(value) = receiver.ingest(&frame).unwrap() {
                        if value["state"] == "saved" {
                            saved = Some(value);
                        }
                    }
                }
                if saved.is_some() {
                    break;
                }
            }
            let value = saved.expect("repair frames must recover missing data");
            assert_eq!(fs::read(value["path"].as_str().unwrap()).unwrap(), original);
            assert_eq!(fs::read_dir(&directory).unwrap().count(), 1);
            fs::remove_dir_all(&directory).unwrap();
        }
    }

    #[test]
    fn oversized_descriptor_is_rejected_before_session_creation() {
        use qr_protocol::session::SessionId;
        use transfer_engine::{SenderConfig, SenderSession};
        let directory =
            std::env::temp_dir().join(format!("airferry-limit-test-{}", std::process::id()));
        let data = vec![0u8; 2048];
        let meta = FileMeta {
            original_size: data.len() as u64,
            compressed_size: data.len() as u64,
            compressed_size_known: true,
            ..Default::default()
        };
        let mut sender =
            SenderSession::new(&data, SessionId(99), SenderConfig::default(), meta).unwrap();
        let mut receiver = Receiver::new(&directory, 1024).unwrap();
        assert!(receiver
            .ingest(&sender.next_frame().unwrap().to_bytes())
            .is_err());
        assert!(receiver.session.is_none());
        assert_eq!(fs::read_dir(&directory).unwrap().count(), 0);
        fs::remove_dir(&directory).unwrap();
    }

    #[test]
    fn interrupted_temporary_file_and_repeated_transfer_do_not_block_reception() {
        use qr_protocol::session::SessionId;
        use transfer_engine::{SenderConfig, SenderSession};
        let directory =
            std::env::temp_dir().join(format!("airferry-restart-test-{}", std::process::id()));
        fs::create_dir(&directory).unwrap();
        let stale = directory.join(".0000000000000000000000000000002a-test.bin.part");
        fs::write(&stale, b"interrupted").unwrap();
        let data = vec![73u8; 4096];
        let meta = FileMeta {
            filename: "test.bin".into(),
            original_size: data.len() as u64,
            compressed_size: data.len() as u64,
            compressed_size_known: true,
            crc32: crc32fast::hash(&data),
            crc32_known: true,
            ..Default::default()
        };
        let mut sender =
            SenderSession::new(&data, SessionId(42), SenderConfig::default(), meta).unwrap();
        for _ in 0..2 {
            let mut receiver = Receiver::new(&directory, 1024 * 1024).unwrap();
            let mut saved = None;
            for _ in 0..100 {
                if let Some(value) = receiver
                    .ingest(&sender.next_frame().unwrap().to_bytes())
                    .unwrap()
                {
                    if value["state"] == "saved" {
                        saved = Some(value);
                        break;
                    }
                }
            }
            let saved =
                saved.expect("Both a first reception and a repeat after restart must succeed");
            assert_eq!(fs::read(saved["path"].as_str().unwrap()).unwrap(), data);
            assert_eq!(fs::read(&stale).unwrap(), b"interrupted");
        }
        fs::remove_dir_all(directory).unwrap();
    }
}
