use super::core::PUSH_PULL_PROTOCOL_ID;
use crate::traits::{MessageMeta, RawSender};

use bytes::{Bytes, BytesMut};
use derive_more::{Constructor, Display};
use futures::{
    channel::mpsc::{unbounded, UnboundedReceiver, UnboundedSender},
    pin_mut,
    stream::Stream,
};
use futures_timer::Delay;
use parking_lot::RwLock;
use protocol::traits::Priority;
use tentacle::{
    builder::MetaBuilder,
    context::ProtocolContextMutRef,
    service::{ProtocolHandle, ProtocolMeta, TargetSession},
    traits::SessionProtocol,
    ProtocolId, SessionId,
};

use std::{
    borrow::Borrow,
    collections::HashSet,
    fmt,
    future::Future,
    hash::{Hash, Hasher},
    ops::{Add, Deref},
    pin::Pin,
    sync::atomic::{AtomicU32, Ordering::SeqCst},
    sync::Arc,
    task::{Context, Poll},
    time::{Duration, Instant},
};

pub const NAME: &str = "chain_pushpull";
pub const SUPPORT_VERSIONS: [&str; 1] = ["0.1"];

pub const MIN_CHUNK_SIZE: u64 = 64 * 1024; // 64KiB
pub const MAX_CACHE_LIFETIME: Duration = Duration::from_secs(60);

static NEXT_PULL_ID: AtomicU32 = AtomicU32::new(0);

#[derive(Debug, Display)]
enum WiredError {
    #[display(fmt = "wired malformat, should at least contains 5 bytes")]
    Malformat,

    #[display(fmt = "wired unknown code {}", _0)]
    UnknownCode(u8),
}

#[derive(Debug, Display)]
enum BadRequestError {
    #[display(fmt = "request decode failed {}", _0)]
    Decode(prost::DecodeError),

    #[display(fmt = "request empty")]
    Empty,

    #[display(fmt = "request no origin hash")]
    NoOriginHash,

    #[display(fmt = "request data not found")]
    NotFound,

    #[display(fmt = "request out of bound")]
    OutOfBound,
}

#[derive(Debug, Display)]
enum BadResponseError {
    #[display(fmt = "response empty")]
    Empty,

    #[display(fmt = "response decode failed {}", _0)]
    Decode(prost::DecodeError),
}

#[derive(Debug, Display)]
enum InternalError {
    #[display(fmt = "encode failed {}", _0)]
    Encode(prost::EncodeError),
}

#[derive(Debug, Display)]
pub enum PullError {
    #[display(fmt = "timeout")]
    Timeout,

    #[display(fmt = "not found")]
    NotFound,

    #[display(fmt = "internal {:?}", _0)]
    Internal(Option<String>),
}

impl std::error::Error for PullError {}

impl From<BadRequestError> for PullError {
    fn from(err: BadRequestError) -> PullError {
        use BadRequestError::*;

        match err {
            NotFound => PullError::NotFound,
            _ => PullError::Internal(Some(err.to_string())),
        }
    }
}

impl From<BadResponseError> for PullError {
    fn from(err: BadResponseError) -> PullError {
        PullError::Internal(Some(err.to_string()))
    }
}

impl From<InternalError> for PullError {
    fn from(err: InternalError) -> PullError {
        PullError::Internal(Some(err.to_string()))
    }
}

#[derive(Clone, PartialEq, Eq, Hash, prost::Message)]
pub struct DataHash {
    #[prost(bytes, tag = "1")]
    inner: Vec<u8>,
}

impl DataHash {
    pub fn new(data: &Bytes) -> Self {
        use ophelia_hasher::Hasher;
        use ophelia_hasher_blake2b::Blake2b;

        let blake2b = Blake2b::new(b"push_pull");
        DataHash {
            inner: blake2b.digest(data).to_bytes().to_vec(),
        }
    }
}

impl fmt::Display for DataHash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", hex::encode(&self.inner))
    }
}

#[derive(prost::Message)]
pub struct DataMeta {
    #[prost(message, tag = "1")]
    pub hash: Option<DataHash>,

    #[prost(uint64, tag = "2")]
    pub length: u64,
}

#[derive(prost::Message)]
pub struct DataChunkReq {
    #[prost(message, tag = "1")]
    pub origin: Option<DataHash>,

    #[prost(uint64, tag = "2")]
    pub start: u64,

    #[prost(uint64, tag = "3")]
    pub end: u64,
}

impl DataChunkReq {
    pub fn new(hash: DataHash, start: u64, end: u64) -> Self {
        DataChunkReq {
            origin: Some(hash),
            start,
            end,
        }
    }
}

#[derive(prost::Message)]
pub struct DataChunkResp {
    #[prost(message, tag = "1")]
    pub origin: Option<DataHash>,

    #[prost(uint64, tag = "2")]
    pub start: u64,

    #[prost(bytes, tag = "3")]
    pub data: Vec<u8>,
}

impl DataChunkResp {
    pub fn new(origin: DataHash, start: u64, data: Bytes) -> Self {
        DataChunkResp {
            origin: Some(origin),
            start,
            data: data.to_vec(),
        }
    }
}

#[derive(prost::Message, Constructor)]
pub struct ErrorMessage {
    #[prost(string, tag = "1")]
    pub msg: String,
}

#[derive(Debug, Display, PartialEq, Eq, Clone, Copy)]
pub enum WiredCode {
    Pull = 0,
    Push = 1,
    NotFound = 10,
    BadRequest = 11,
    Internal = 12,
}

impl WiredCode {
    fn try_from(code: u8) -> Result<Self, WiredError> {
        let code = match code {
            0 => WiredCode::Pull,
            1 => WiredCode::Push,
            10 => WiredCode::NotFound,
            11 => WiredCode::BadRequest,
            12 => WiredCode::Internal,
            _ => return Err(WiredError::UnknownCode(code)),
        };

        Ok(code)
    }
}

#[derive(Debug)]
pub struct WiredMessage {
    pub code:    WiredCode,
    pub pull_id: u32,
    pub data:    Bytes,
}

impl WiredMessage {
    fn new_err(code: WiredCode, pull_id: u32) -> Self {
        WiredMessage {
            code,
            pull_id,
            data: Bytes::new(),
        }
    }

    fn new_err_with_msg(code: WiredCode, pull_id: u32, err_msg: ErrorMessage) -> Self {
        use prost::Message;

        let mut buf = BytesMut::with_capacity(err_msg.encoded_len());
        let data = match err_msg.encode(&mut buf) {
            Ok(_) => buf.freeze(),
            Err(e) => {
                log::warn!("fail to encode error message {}", e);
                Bytes::new()
            }
        };

        WiredMessage {
            code,
            pull_id,
            data,
        }
    }

    fn new_req(req: DataChunkReq, pull_id: u32) -> Result<Self, PullError> {
        use prost::Message;

        let mut buf = BytesMut::with_capacity(req.encoded_len());
        req.encode(&mut buf).map_err(InternalError::Encode)?;

        let msg = WiredMessage {
            code: WiredCode::Pull,
            pull_id,
            data: buf.freeze(),
        };

        Ok(msg)
    }

    fn new_resp(resp: DataChunkResp, pull_id: u32) -> Self {
        use prost::Message;

        let mut buf = BytesMut::with_capacity(resp.encoded_len());
        match resp.encode(&mut buf) {
            Ok(_) => WiredMessage {
                code: WiredCode::Push,
                pull_id,
                data: buf.freeze(),
            },
            Err(err) => {
                let err_msg = ErrorMessage::new(InternalError::Encode(err).to_string());
                WiredMessage::new_err_with_msg(WiredCode::Internal, pull_id, err_msg)
            }
        }
    }

    fn to_bytes(&self) -> Bytes {
        let mut buf = BytesMut::with_capacity(5 + self.data.len());
        buf.extend_from_slice(&[self.code as u8]);
        buf.extend_from_slice(&self.pull_id.to_be_bytes());
        buf.extend_from_slice(self.data.as_ref());

        buf.freeze()
    }

    fn parse(mut data: Bytes) -> Result<Self, WiredError> {
        // Ensure that we have at least one byte
        if data.len() < 5 {
            return Err(WiredError::Malformat);
        }

        let code = WiredCode::try_from(data.split_to(1).as_ref()[0])?;

        let mut id_bytes = [0u8; 4];
        id_bytes.copy_from_slice(data.split_to(4).as_ref());

        let pull_id = u32::from_be_bytes(id_bytes);

        let msg = WiredMessage {
            code,
            pull_id,
            data,
        };

        Ok(msg)
    }
}

#[derive(Debug)]
pub struct CachedData {
    hash: DataHash,
    data: Bytes,
}

impl CachedData {
    pub fn new(data: Bytes) -> Self {
        let hash = DataHash::new(&data);
        CachedData { hash, data }
    }
}

impl Borrow<DataHash> for CachedData {
    fn borrow(&self) -> &DataHash {
        &self.hash
    }
}

impl PartialEq for CachedData {
    fn eq(&self, other: &CachedData) -> bool {
        self.hash == other.hash
    }
}

impl Eq for CachedData {}

impl Hash for CachedData {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.hash.hash(state)
    }
}

impl Deref for CachedData {
    type Target = Bytes;

    fn deref(&self) -> &Self::Target {
        &self.data
    }
}

#[derive(Debug, Clone)]
pub struct Cache(Arc<RwLock<HashSet<CachedData>>>);

impl Cache {
    fn insert(&self, data: Bytes) -> DataHash {
        let cached_data = CachedData::new(data);
        let hash = cached_data.hash.clone();
        self.0.write().insert(cached_data);
        hash
    }

    fn get(
        &self,
        hash: &DataHash,
        start: usize,
        end: usize,
    ) -> Result<Option<Bytes>, BadRequestError> {
        let checked_slice = |data: &CachedData| -> _ {
            if start >= data.len() || end > data.len() {
                Err(BadRequestError::OutOfBound)
            } else {
                Ok(data.slice(start..end))
            }
        };

        let cache = self.0.read();
        cache.get(hash).map(checked_slice).transpose()
    }

    fn remove(&self, hash: &DataHash) {
        self.0.write().remove(hash);
    }
}

impl Default for Cache {
    fn default() -> Self {
        Cache(Default::default())
    }
}

#[derive(Debug, PartialEq, Eq, Hash, Clone, Copy, Constructor)]
pub struct SenderId {
    sid:     SessionId,
    pull_id: u32,
}

#[derive(Clone, Constructor)]
pub struct ChunkSender {
    id: SenderId,
    tx: UnboundedSender<Result<DataChunkResp, PullError>>,
}

impl Borrow<SenderId> for ChunkSender {
    fn borrow(&self) -> &SenderId {
        &self.id
    }
}

impl PartialEq for ChunkSender {
    fn eq(&self, other: &ChunkSender) -> bool {
        self.id == other.id
    }
}

impl Eq for ChunkSender {}

impl Hash for ChunkSender {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.id.hash(state)
    }
}

impl Deref for ChunkSender {
    type Target = UnboundedSender<Result<DataChunkResp, PullError>>;

    fn deref(&self) -> &Self::Target {
        &self.tx
    }
}

#[derive(Clone)]
pub struct ChunkTxs(Arc<RwLock<HashSet<ChunkSender>>>);

impl Default for ChunkTxs {
    fn default() -> Self {
        ChunkTxs(Default::default())
    }
}

impl ChunkTxs {
    pub fn insert(&self, tx: ChunkSender) {
        self.0.write().insert(tx);
    }

    pub fn get(&self, tx_id: SenderId) -> Option<ChunkSender> {
        self.0.read().get(&tx_id).cloned()
    }

    pub fn remove(&self, tx_id: SenderId) {
        self.0.write().remove(&tx_id);
    }
}

#[derive(Debug, Clone, Copy, Constructor)]
struct ChunkRange {
    start: u64,
    end:   u64,
}

#[derive(Debug)]
struct MissingChunks(Vec<ChunkRange>);

impl MissingChunks {
    pub fn new() -> Self {
        MissingChunks(Vec::with_capacity(4))
    }

    pub fn is_complete(&self) -> bool {
        self.0.is_empty()
    }

    pub fn insert(&mut self, ranges: &[ChunkRange]) {
        self.0.extend_from_slice(ranges)
    }

    pub fn split_half(&mut self) -> Vec<ChunkRange> {
        self.0 = self
            .iter()
            .map(|r| {
                let middle = (r.end - r.start) / 2;
                vec![
                    ChunkRange::new(r.start, middle),
                    ChunkRange::new(middle, r.end),
                ]
            })
            .flatten()
            .collect::<Vec<_>>();

        self.0.clone()
    }

    // Note: only remove ranges if given range is greater than or equal.
    pub fn complete_range(&mut self, range: ChunkRange) {
        self.0 = self
            .iter()
            .filter(|r| r.start < range.start && r.end > range.end)
            .cloned()
            .collect()
    }
}

impl Deref for MissingChunks {
    type Target = Vec<ChunkRange>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[derive(Debug, Clone, Copy)]
pub struct PullTimeout {
    pub chunk: Duration,
    pub max:   Duration,
}

pub struct PullData<S: RawSender + Unpin + 'static> {
    sid:          SessionId,
    pull_id:      u32,
    data_hash:    DataHash,
    data_len:     u64,
    data_chunks:  Vec<DataChunkResp>,
    timeout_conf: PullTimeout,
    missings:     MissingChunks,

    network:     S,
    timeout:     Delay,
    max_timeout: Delay,
    chunk_rx:    UnboundedReceiver<Result<DataChunkResp, PullError>>,
    chunk_txs:   ChunkTxs,
}

impl<S: RawSender + Unpin + 'static> PullData<S> {
    fn send_chunk_req(&self, range: ChunkRange) -> Result<(), PullError> {
        let req = DataChunkReq::new(self.data_hash.clone(), range.start, range.end);
        let msg = WiredMessage::new_req(req, self.pull_id)?.to_bytes();

        let meta = MessageMeta {
            sessions: TargetSession::Single(self.sid),
            protocol: PUSH_PULL_PROTOCOL_ID.into(),
            priority: Priority::High,
        };
        log::debug!("protocol id {}", PUSH_PULL_PROTOCOL_ID);

        self.network
            .raw_send(meta, msg)
            .map_err(|e| PullError::Internal(Some(e.to_string())))?;

        Ok(())
    }
}

impl<S: RawSender + Unpin + 'static> Future for PullData<S> {
    type Output = Result<Bytes, PullError>;

    fn poll(mut self: Pin<&mut Self>, ctx: &mut Context<'_>) -> Poll<Self::Output> {
        let max_timeout = &mut self.as_mut().max_timeout;
        pin_mut!(max_timeout);

        if let Poll::Ready(_) = max_timeout.poll(ctx) {
            log::info!("pull {} reach max timeout", self.data_hash);
            return Poll::Ready(Err(PullError::Timeout));
        }

        let chunk_timeout = &mut self.as_mut().timeout;
        pin_mut!(chunk_timeout);

        // Pull chunk timeout, split chunk size into half, then try pull again.
        if let Poll::Ready(_) = chunk_timeout.poll(ctx) {
            log::info!("pull {} chunk timeout, split half", self.data_hash);

            let chunk_ranges = self.missings.split_half();
            for range in chunk_ranges {
                if let Err(e) = self.send_chunk_req(range) {
                    return Poll::Ready(Err(e));
                }
            }
            log::debug!("{} chunk missings {:?}", self.data_hash, self.missings);

            let next_time = Instant::now().add(self.timeout_conf.chunk);
            self.timeout.reset(next_time);
        }

        loop {
            let chunk_rx = &mut self.as_mut().chunk_rx;
            pin_mut!(chunk_rx);

            let resp = match chunk_rx.poll_next(ctx) {
                Poll::Pending => return Poll::Pending,
                Poll::Ready(Some(resp)) => resp,
                Poll::Ready(None) => {
                    let err = PullError::Internal(Some("chunk tx dropped".to_owned()));
                    return Poll::Ready(Err(err));
                }
            };

            let chunk = match resp {
                Ok(chunk) => chunk,
                Err(err) => return Poll::Ready(Err(err)),
            };

            log::debug!(
                "receive {:?} chunk start {} len {}",
                chunk.origin,
                chunk.start,
                chunk.data.len()
            );

            if chunk.start >= self.data_len
                || chunk.data.len() as u64 > self.data_len
                || chunk.start + chunk.data.len() as u64 > self.data_len
            {
                log::warn!("got malformat chunk from session {}", self.sid);
                continue;
            }

            let chunk_range = ChunkRange::new(chunk.start, chunk.start + chunk.data.len() as u64);
            self.missings.complete_range(chunk_range);
            self.data_chunks.push(chunk);

            log::debug!("check complete, missing {:?}", self.missings);
            if self.missings.is_complete() {
                log::debug!("{} pull complete", self.data_hash);
                break;
            }
        }

        let mut data_buf = vec![0u8; self.data_len as usize];
        for chunk in self.data_chunks.iter() {
            let start = chunk.start as usize;
            let end = start + chunk.data.len();
            data_buf[start..end].copy_from_slice(chunk.data.as_slice())
        }

        let data = Bytes::copy_from_slice(data_buf.as_slice());
        if DataHash::new(&data) != self.data_hash {
            let err = PullError::Internal(Some("corrupted data".to_owned()));
            Poll::Ready(Err(err))
        } else {
            Poll::Ready(Ok(data))
        }
    }
}

impl<S: RawSender + Unpin + 'static> Drop for PullData<S> {
    fn drop(&mut self) {
        let tx_id = SenderId::new(self.sid, self.pull_id);
        self.chunk_txs.remove(tx_id);
    }
}

struct CacheCleaner {
    data_hash: DataHash,
    timeout:   Delay,
    cache:     Cache,
}

impl Future for CacheCleaner {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, ctx: &mut Context<'_>) -> Poll<Self::Output> {
        let timeout = &mut self.as_mut().timeout;
        pin_mut!(timeout);

        match timeout.poll(ctx) {
            Poll::Pending => Poll::Pending,
            Poll::Ready(_) => {
                self.cache.remove(&self.data_hash);
                Poll::Ready(())
            }
        }
    }
}

#[derive(Clone)]
pub struct PushPull {
    pub(crate) cache: Cache,
    pub(crate) txs:   ChunkTxs,
}

impl Default for PushPull {
    fn default() -> Self {
        PushPull {
            cache: Default::default(),
            txs:   Default::default(),
        }
    }
}

impl PushPull {
    pub fn build_meta(self, protocol_id: ProtocolId) -> ProtocolMeta {
        MetaBuilder::new()
            .id(protocol_id)
            .name(name!(NAME))
            .support_versions(support_versions!(SUPPORT_VERSIONS))
            .session_handle(move || ProtocolHandle::Callback(Box::new(self.clone())))
            .build()
    }

    pub fn cache_data(&self, data: Bytes) -> DataMeta {
        let length = data.len() as u64;
        let hash = self.cache.insert(data);

        tokio::spawn(CacheCleaner {
            data_hash: hash.clone(),
            timeout:   Delay::new(MAX_CACHE_LIFETIME),
            cache:     self.cache.clone(),
        });

        DataMeta {
            hash: Some(hash),
            length,
        }
    }

    pub fn pull<S: RawSender + Unpin + 'static>(
        &self,
        network: S,
        sid: SessionId,
        timeout: PullTimeout,
        data_hash: DataHash,
        data_len: u64,
    ) -> Result<PullData<S>, PullError> {
        let pull_id = NEXT_PULL_ID.fetch_add(1, SeqCst);

        let mut missings = MissingChunks::new();
        let reqs = if data_len <= MIN_CHUNK_SIZE {
            missings.insert(&[ChunkRange::new(0, data_len)]);
            vec![DataChunkReq::new(data_hash.clone(), 0, data_len)]
        } else {
            let binary_start = data_len / 2;

            let first_half = DataChunkReq::new(data_hash.clone(), 0, binary_start);
            let second_half = DataChunkReq::new(data_hash.clone(), binary_start, data_len);

            missings.insert(&[
                ChunkRange::new(0, binary_start),
                ChunkRange::new(binary_start, data_len),
            ]);

            vec![first_half, second_half]
        };

        for req in reqs {
            let meta = MessageMeta {
                sessions: TargetSession::Single(sid),
                protocol: PUSH_PULL_PROTOCOL_ID.into(),
                priority: Priority::High,
            };

            let msg = WiredMessage::new_req(req, pull_id)?.to_bytes();
            network
                .raw_send(meta, msg)
                .map_err(|e| PullError::Internal(Some(e.to_string())))?;
        }

        log::debug!("send all pull request");

        let (tx, rx) = unbounded();
        let tx_id = SenderId::new(sid, pull_id);
        let chunk_tx = ChunkSender::new(tx_id, tx);
        self.txs.insert(chunk_tx);

        let pull = PullData {
            sid,
            pull_id,
            data_hash,
            data_len,
            data_chunks: Vec::new(),
            timeout_conf: timeout,
            missings,

            network,
            timeout: Delay::new(timeout.chunk),
            max_timeout: Delay::new(timeout.max),
            chunk_rx: rx,
            chunk_txs: self.txs.clone(),
        };

        Ok(pull)
    }

    fn handle_chunk_pull(&self, data: Bytes) -> Result<DataChunkResp, BadRequestError> {
        use prost::Message;

        if data.is_empty() {
            return Err(BadRequestError::Empty);
        }

        let DataChunkReq { origin, start, end } =
            DataChunkReq::decode(data).map_err(BadRequestError::Decode)?;
        log::debug!("pull request for {:?} start {} end {}", origin, start, end);

        let origin = origin.ok_or_else(|| BadRequestError::NoOriginHash)?;
        let chunk = match self.cache.get(&origin, start as usize, end as usize)? {
            Some(chunk) => DataChunkResp::new(origin.clone(), start, chunk),
            None => return Err(BadRequestError::NotFound),
        };

        Ok(chunk)
    }

    fn handle_chunk_push(&self, data: Bytes) -> Result<DataChunkResp, BadResponseError> {
        use prost::Message;

        if data.is_empty() {
            return Err(BadResponseError::Empty);
        }

        let chunk = DataChunkResp::decode(data).map_err(BadResponseError::Decode)?;

        log::debug!(
            "push for {:?} start {} len {}",
            chunk.origin,
            chunk.start,
            chunk.data.len()
        );

        Ok(chunk)
    }
}

impl SessionProtocol for PushPull {
    fn received(&mut self, ctx: ProtocolContextMutRef, data: Bytes) {
        use prost::Message;

        let remote_addr = &ctx.session.address;
        let WiredMessage {
            code,
            pull_id,
            data,
        } = match WiredMessage::parse(data) {
            Ok(c) => c,
            Err(e) => {
                log::warn!("invalid wired {} from {}, drop !!!", e, remote_addr);
                return;
            }
        };

        macro_rules! chunk_sender {
            () => {{
                let tx_id = SenderId::new(ctx.session.id, pull_id);
                match self.txs.get(tx_id) {
                    Some(tx) => tx,
                    None => {
                        log::warn!(
                            "chunk sender not found, may be wrong, timeout or already completed"
                        );
                        return;
                    }
                }
            }};
        }

        match code {
            // Got pull chunk request from remote peer
            WiredCode::Pull => {
                let wired_message = match self.handle_chunk_pull(data) {
                    Ok(chunk_resp) => WiredMessage::new_resp(chunk_resp, pull_id),
                    Err(BadRequestError::NotFound) => {
                        WiredMessage::new_err(WiredCode::NotFound, pull_id)
                    }
                    Err(e) => WiredMessage::new_err_with_msg(
                        WiredCode::BadRequest,
                        pull_id,
                        ErrorMessage::new(e.to_string()),
                    ),
                };

                if let Err(e) = ctx.send_message(wired_message.to_bytes()) {
                    log::warn!("send to {} fail {}", remote_addr, e);
                }
            }
            // Got our required chunk from remote peer
            WiredCode::Push => {
                let chunk_tx = chunk_sender!();
                let ret = self.handle_chunk_push(data).map_err(PullError::from);

                if let Err(e) = chunk_tx.unbounded_send(ret.into()) {
                    log::warn!("chunk tx fail {}", e);
                }
            }
            // Error on pull chunk
            WiredCode::NotFound | WiredCode::BadRequest | WiredCode::Internal => {
                let chunk_tx = chunk_sender!();

                let err = if code == WiredCode::NotFound {
                    PullError::NotFound
                } else {
                    let mut err_msg = None;
                    if !data.is_empty() {
                        match ErrorMessage::decode(data) {
                            Ok(err) => err_msg = Some(err.msg),
                            Err(e) => {
                                log::warn!("decode error message fail {}", e);
                            }
                        }
                    }
                    PullError::Internal(err_msg)
                };

                if let Err(e) = chunk_tx.unbounded_send(Err(err)) {
                    log::warn!("chunk tx fail {}", e);
                }
            }
        }
    }
}
