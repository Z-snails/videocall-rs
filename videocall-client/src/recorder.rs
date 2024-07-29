use std::{future::Future, marker::PhantomData, rc::Rc};

use gloo::console;
use js_sys::{JsString, Object, Reflect, Uint8Array};
use rexie::{ObjectStore, Rexie, TransactionMode};
use wasm_bindgen::{JsValue, UnwrapThrowExt};
use web_sys::{EncodedAudioChunk, EncodedAudioChunkType, EncodedVideoChunk, EncodedVideoChunkType};

fn try_spawn<E: std::error::Error, F: Future<Output = std::result::Result<(), E>> + 'static>(f: F) {
    wasm_bindgen_futures::spawn_local(async move {
        match f.await {
            Ok(_) => {}
            Err(err) => console::error!(format!("{err:?}")),
        }
    })
}

#[derive(Clone, Debug)]
pub struct IdbRecorder {
    database: Rc<Rexie>,
    sender: async_channel::Sender<SomeChunkId>,
    receiver: async_channel::Receiver<SomeChunkId>,
}

pub type Error = rexie::Error;
pub type Result<T> = rexie::Result<T>;

impl PartialEq for IdbRecorder {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.database, &other.database)
    }
}

impl IdbRecorder {
    const VIDEO_STORE: &'static str = "video";
    const AUDIO_STORE: &'static str = "audio";

    pub async fn new(name: &str) -> rexie::Result<IdbRecorder> {
        let database = Rexie::builder(name)
            .version(1)
            .add_object_store(ObjectStore::new(Self::VIDEO_STORE).auto_increment(true))
            .add_object_store(ObjectStore::new(Self::AUDIO_STORE).auto_increment(true))
            .build()
            .await?;
        let (sender, receiver) = async_channel::unbounded();

        Ok(IdbRecorder {
            database: Rc::new(database),
            sender,
            receiver,
        })
    }

    pub(crate) fn record_video(&self, chunk: &EncodedVideoChunk) {
        let this = self.clone();
        let chunk = EncodedChunk::from(chunk);
        try_spawn::<rexie::Error, _>(async move {
            let trans = this
                .database
                .transaction(&[IdbRecorder::VIDEO_STORE], TransactionMode::ReadWrite)?;
            let video = trans.store(IdbRecorder::VIDEO_STORE)?;
            let id = video.add(&chunk.as_object(), None).await?;
            trans.done().await?;
            let _ = this.sender.send(SomeChunkId::Video(ChunkId::new(id))).await;
            Ok(())
        })
    }

    pub(crate) fn record_audio(&self, chunk: &EncodedAudioChunk) {
        let this = self.clone();
        let chunk = EncodedChunk::from(chunk);
        try_spawn::<rexie::Error, _>(async move {
            let trans = this
                .database
                .transaction(&[IdbRecorder::AUDIO_STORE], TransactionMode::ReadWrite)?;
            let audio = trans.store(IdbRecorder::AUDIO_STORE)?;
            let id = audio.add(&chunk.as_object(), None).await?;
            trans.done().await?;
            gloo::console::log!("Added to db", id);
            let _ = this.sender.send(SomeChunkId::Audio(ChunkId::new(id))).await;
            Ok(())
        })
    }

    // pub fn start_upload(&self) {
    //     let this = self.clone();
    //     wasm_bindgen_futures::spawn_local(async move {
    //         while let Ok(id) = this.receiver.recv().await {

    //         }
    //     });
    // }
}

pub struct ChunkId<T> {
    id: JsValue,
    phantom: PhantomData<T>,
}

impl<T> Clone for ChunkId<T> {
    fn clone(&self) -> Self {
        Self {
            id: self.id.clone(),
            phantom: PhantomData,
        }
    }
}

impl<T> ChunkId<T> {
    fn new(id: JsValue) -> Self {
        Self {
            id,
            phantom: PhantomData,
        }
    }
}

pub type VideoChunkId = ChunkId<EncodedVideoChunk>;
pub type AudioChunkId = ChunkId<EncodedAudioChunk>;

pub enum SomeChunkId {
    Video(VideoChunkId),
    Audio(AudioChunkId),
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ChunkKind {
    Video,
    Audio,
}

impl From<ChunkKind> for JsString {
    fn from(value: ChunkKind) -> Self {
        match value {
            ChunkKind::Video => "video".into(),
            ChunkKind::Audio => "audio".into(),
        }
    }
}

/// The type of a video frame
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum FrameType {
    Key,
    Delta,
}

impl From<FrameType> for JsString {
    fn from(value: FrameType) -> Self {
        match value {
            FrameType::Key => "key".into(),
            FrameType::Delta => "delta".into(),
        }
    }
}

impl From<EncodedVideoChunkType> for FrameType {
    fn from(value: EncodedVideoChunkType) -> Self {
        match value {
            EncodedVideoChunkType::Key => FrameType::Key,
            EncodedVideoChunkType::Delta => FrameType::Delta,
            _ => panic!("Unknown EncodedVideoChunkType"),
        }
    }
}

impl From<EncodedAudioChunkType> for FrameType {
    fn from(value: EncodedAudioChunkType) -> Self {
        match value {
            EncodedAudioChunkType::Key => FrameType::Key,
            EncodedAudioChunkType::Delta => FrameType::Delta,
            _ => panic!("Unknown EncodedAudioChunkType"),
        }
    }
}

/// An encoded video or audio chunk
pub struct EncodedChunk {
    /// Video or audio
    kind: ChunkKind,
    // Keyframe or delta
    r#type: FrameType,
    /// The timestamp in the video in ms
    timestamp: f64,
    /// The length of the chunk in ms
    duration: f64,
    /// The data
    array: Uint8Array,
}

impl From<&EncodedVideoChunk> for EncodedChunk {
    fn from(value: &EncodedVideoChunk) -> Self {
        let array = Uint8Array::new_with_length(value.byte_length());
        value.copy_to_with_buffer_source(&array);
        Self {
            kind: ChunkKind::Video,
            r#type: value.type_().into(),
            timestamp: value.timestamp(),
            duration: value.duration().expect_throw("Expected duration"),
            array,
        }
    }
}

impl From<&EncodedAudioChunk> for EncodedChunk {
    fn from(value: &EncodedAudioChunk) -> Self {
        let array = Uint8Array::new_with_length(value.byte_length());
        value.copy_to_with_buffer_source(&array);
        Self {
            kind: ChunkKind::Audio,
            r#type: value.type_().into(),
            timestamp: value.timestamp(),
            duration: value.duration().expect_throw("Expected duration"),
            array,
        }
    }
}

impl EncodedChunk {
    #[allow(unused_must_use)]
    fn as_object(&self) -> Object {
        let object = Object::new();
        Reflect::set(&object, &"kind".into(), &JsString::from(self.kind));
        Reflect::set(&object, &"type".into(), &JsString::from(self.r#type));
        Reflect::set(
            &object,
            &"timestamp".into(),
            &JsValue::from_f64(self.timestamp),
        );
        Reflect::set(
            &object,
            &"duration".into(),
            &JsValue::from_f64(self.duration),
        );
        Reflect::set(&object, &"data".into(), &self.array);
        object
    }
}
