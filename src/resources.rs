//! Loads resource files into the executable

/// An embedded media file with the metadata required by messaging adapters.
#[derive(Clone, Copy)]
pub struct EmbeddedMedia {
    pub file_name: &'static str,
    pub mime_type: &'static str,
    pub bytes: &'static [u8],
}

/// Progress images
pub const FRAMES: &[&[u8]] = &[
    include_bytes!("../resources/frame0.png"),
    include_bytes!("../resources/frame1.png"),
    include_bytes!("../resources/frame2.png"),
    include_bytes!("../resources/frame3.png"),
];

// The Document
pub const RESULT: &[u8] = include_bytes!("../resources/result.zip");

/// Media shared by the WhatsApp and Telegram demoscenes.
pub const DEMO_IMAGE: EmbeddedMedia = EmbeddedMedia {
    file_name: "demo_image.png",
    mime_type: "image/png",
    bytes: include_bytes!("../resources/frame0.png"),
};
pub const DEMO_AUDIO: EmbeddedMedia = EmbeddedMedia {
    file_name: "demo_audio.mp3",
    mime_type: "audio/mpeg",
    bytes: include_bytes!("../resources/demo_audio.mp3"),
};
pub const DEMO_VOICE: EmbeddedMedia = EmbeddedMedia {
    file_name: "demo_voice.ogg",
    mime_type: "audio/ogg",
    bytes: include_bytes!("../resources/demo_voice.ogg"),
};
pub const DEMO_VIDEO: EmbeddedMedia = EmbeddedMedia {
    file_name: "demo_video.mp4",
    mime_type: "video/mp4",
    bytes: include_bytes!("../resources/demo_video.mp4"),
};
pub const DEMO_STICKER: EmbeddedMedia = EmbeddedMedia {
    file_name: "demo_sticker.webp",
    mime_type: "image/webp",
    bytes: include_bytes!("../resources/demo_sticker.webp"),
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn demo_media_have_expected_container_signatures() {
        assert!(
            DEMO_IMAGE
                .bytes
                .starts_with(b"\x89PNG\r\n\x1a\n")
        );
        assert!(
            DEMO_AUDIO
                .bytes
                .starts_with(&[0xff, 0xfb])
        );
        assert!(
            DEMO_VOICE
                .bytes
                .starts_with(b"OggS")
        );
        assert_eq!(&DEMO_VIDEO.bytes[4..8], b"ftyp");
        assert!(
            DEMO_STICKER
                .bytes
                .starts_with(b"RIFF")
        );
        assert_eq!(&DEMO_STICKER.bytes[8..12], b"WEBP");
    }

    #[test]
    fn demo_media_metadata_match_file_extensions() {
        assert_eq!((DEMO_AUDIO.file_name, DEMO_AUDIO.mime_type), ("demo_audio.mp3", "audio/mpeg"));
        assert_eq!((DEMO_VOICE.file_name, DEMO_VOICE.mime_type), ("demo_voice.ogg", "audio/ogg"));
        assert_eq!((DEMO_VIDEO.file_name, DEMO_VIDEO.mime_type), ("demo_video.mp4", "video/mp4"));
        assert_eq!((DEMO_STICKER.file_name, DEMO_STICKER.mime_type), ("demo_sticker.webp", "image/webp"));
    }
}
