//! Loads resource files into the executable

/// Progress images
pub const FRAMES: &[&[u8]] = &[
    include_bytes!("../resources/frame0.png"),
    include_bytes!("../resources/frame1.png"),
    include_bytes!("../resources/frame2.png"),
    include_bytes!("../resources/frame3.png"),
];

// The Document
pub const RESULT: &[u8] = include_bytes!("../resources/result.zip");
