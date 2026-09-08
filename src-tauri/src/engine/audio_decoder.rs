//! Pure Rust audio/video decoding via symphonia.
//!
//! Replaces FFmpeg-based audio extraction with packet-by-packet streaming
//! decode, enabling progressive VAD+ASR without buffering the entire file.

use crate::errors::{AppError, AppResult};
use sherpa_onnx::LinearResampler;
use std::fs::File;
use std::io::Write;
use symphonia::core::audio::{AudioBufferRef, SampleBuffer};
use symphonia::core::codecs::{Decoder, CODEC_TYPE_NULL};
use symphonia::core::formats::FormatReader;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::probe::Hint;

/// Opened audio source: (format reader, decoder, track id, channel count, sample rate).
pub type OpenedAudio =
    (Box<dyn FormatReader>, Box<dyn Decoder>, u32, usize, u32);

/// Open an audio/video file and return the format reader, decoder, and track info.
/// Supports MP3, FLAC, AAC, OGG, WAV, MP4, MKV, WebM, AIFF, and more via symphonia.
pub fn open_audio_file(path: &str) -> AppResult<OpenedAudio> {
    let src = File::open(path).map_err(|e| {
        AppError::Io(std::io::Error::other(format!("Cannot open file: {e}")))
    })?;
    let mss = MediaSourceStream::new(Box::new(src), Default::default());

    let mut hint = Hint::new();
    if let Some(ext) = std::path::Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
    {
        hint.with_extension(ext);
    }

    let probed = symphonia::default::get_probe()
        .format(&hint, mss, &Default::default(), &Default::default())
        .map_err(|e| AppError::UnsupportedFormat(format!("Unsupported format: {e}")))?;

    let format = probed.format;

    // Find the first *audio* track (not video).
    // Video files (MP4, MKV, etc.) have multiple tracks; we must skip the
    // video track(s) and pick the audio one.
    //
    // Strategy: iterate tracks, skip CODEC_TYPE_NULL (video codecs with
    // audio-only symphonia features will be NULL), try to create a decoder.
    // If decoder creation succeeds, we have an audio track.
    // sample_rate and channels default to 16000/1 if not available in metadata
    // (MP4/MKV containers often don't report them during probe).
    let (track_id, sample_rate, num_channels, track_codec_params) = {
        let mut audio_track = None;
        let track_count = format.tracks().len();
        log::info!("[open_audio_file] found {track_count} tracks in file");
        for track in format.tracks().iter() {
            let codec = track.codec_params.codec;
            let sr = track.codec_params.sample_rate;
            let ch = track.codec_params.channels.as_ref().map(|c| c.count());
            log::info!(
                "[open_audio_file] track {:?}: codec={codec:?}, sample_rate={sr:?}, channels={ch:?}",
                track.id
            );
            if codec == CODEC_TYPE_NULL {
                log::debug!(
                    "[open_audio_file] skip track {:?}: CODEC_TYPE_NULL",
                    track.id
                );
                continue;
            }

            // Try to create a decoder — with audio-only symphonia features,
            // video codecs will fail here.
            match symphonia::default::get_codecs().make(&track.codec_params, &Default::default()) {
                Ok(dec) => {
                    let sr = track.codec_params.sample_rate.unwrap_or(16000);
                    let ch = track.codec_params.channels.map(|c| c.count()).unwrap_or(1);
                    audio_track = Some((track.id, sr, ch, dec));
                    log::info!(
                        "[open_audio_file] selected track {:?}: codec={codec:?}, sample_rate={sr}, channels={ch}",
                        track.id
                    );
                    break;
                }
                Err(e) => {
                    log::debug!(
                        "[open_audio_file] skip track {:?}: decoder creation failed: {e}",
                        track.id
                    );
                    continue;
                }
            }
        }
        audio_track.ok_or_else(|| {
            log::error!(
                "[open_audio_file] no supported audio track. Tracks: {:?}",
                format
                    .tracks()
                    .iter()
                    .map(|t| format!("{:?}: codec={:?}", t.id, t.codec_params.codec))
                    .collect::<Vec<_>>()
            );
            AppError::UnsupportedFormat("No supported audio track found".into())
        })?
    };

    let decoder = track_codec_params;

    Ok((format, decoder, track_id, num_channels, sample_rate))
}

/// Decode interleaved AudioBufferRef to mono f32 samples.
pub fn decode_to_mono_f32(decoded: &AudioBufferRef, num_channels: usize) -> Vec<f32> {
    let mut buf = SampleBuffer::<f32>::new(decoded.capacity() as u64, *decoded.spec());
    buf.copy_interleaved_ref(decoded.clone());
    let all = buf.samples();

    if num_channels == 1 {
        return all.to_vec();
    }

    let nc = num_channels;
    let num_frames = all.len() / nc;
    let mut mono = Vec::with_capacity(num_frames);
    for frame in 0..num_frames {
        let mut sum = 0.0f32;
        for ch in 0..nc {
            sum += all[frame * nc + ch];
        }
        mono.push(sum / nc as f32);
    }
    mono
}

/// Create a resampler for converting native sample rate to 16 kHz.
pub fn create_resampler(native_rate: u32) -> AppResult<Option<LinearResampler>> {
    if native_rate != 16000 {
        Ok(Some(
            LinearResampler::create(native_rate as i32, 16000).ok_or_else(|| {
                AppError::Transcription(format!("Failed to create resampler for {native_rate} Hz"))
            })?,
        ))
    } else {
        Ok(None)
    }
}

/// Write mono f32 PCM samples as a 16-bit WAV file at 16 kHz.
pub fn write_wav(path: &str, samples: &[f32]) -> AppResult<()> {
    let num_samples = samples.len() as u32;
    let byte_rate = 16000u32 * 2;
    let data_size = num_samples * 2;
    let file_size = 36 + data_size;

    let f = File::create(path).map_err(AppError::Io)?;
    let mut w = std::io::BufWriter::new(f);

    w.write_all(b"RIFF").map_err(AppError::Io)?;
    w.write_all(&file_size.to_le_bytes())
        .map_err(AppError::Io)?;
    w.write_all(b"WAVE").map_err(AppError::Io)?;
    w.write_all(b"fmt ").map_err(AppError::Io)?;
    w.write_all(&16u32.to_le_bytes()).map_err(AppError::Io)?;
    w.write_all(&1u16.to_le_bytes()).map_err(AppError::Io)?;
    w.write_all(&1u16.to_le_bytes()).map_err(AppError::Io)?;
    w.write_all(&16000u32.to_le_bytes()).map_err(AppError::Io)?;
    w.write_all(&byte_rate.to_le_bytes())
        .map_err(AppError::Io)?;
    w.write_all(&2u16.to_le_bytes()).map_err(AppError::Io)?;
    w.write_all(&16u16.to_le_bytes()).map_err(AppError::Io)?;
    w.write_all(b"data").map_err(AppError::Io)?;
    w.write_all(&data_size.to_le_bytes())
        .map_err(AppError::Io)?;
    for &s in samples {
        let clamped = s.clamp(-1.0, 1.0);
        let pcm = (clamped * 32767.0) as i16;
        w.write_all(&pcm.to_le_bytes()).map_err(AppError::Io)?;
    }
    w.flush().map_err(AppError::Io)?;

    Ok(())
}

/// Decode audio from the file for a specific time range (in seconds at 16 kHz).
/// Only decodes up to `end`, avoiding loading the entire file.
pub fn decode_time_range(path: &str, start: f32, end: f32) -> AppResult<Vec<f32>> {
    let (mut format_reader, mut decoder, track_id, num_channels_hint, _native_rate_hint) =
        open_audio_file(path)?;

    let mut resampler: Option<LinearResampler> = None;
    let mut actual_native_rate: u32;
    let mut actual_num_channels: usize = num_channels_hint;
    let mut first_frame_decoded = false;

    let rate_16k = 16000.0;
    let start_sample = (start * rate_16k) as usize;
    let end_sample = (end * rate_16k) as usize;
    let mut result: Vec<f32> = Vec::new();
    let mut total_16k_samples: usize = 0;

    loop {
        if total_16k_samples >= end_sample {
            break;
        }

        let packet = match format_reader.next_packet() {
            Ok(p) => p,
            Err(symphonia::core::errors::Error::IoError(e))
                if e.kind() == std::io::ErrorKind::UnexpectedEof =>
            {
                break;
            }
            Err(symphonia::core::errors::Error::ResetRequired) => break,
            Err(err) => return Err(AppError::Transcription(format!("Read error: {err}"))),
        };

        if packet.track_id() != track_id {
            continue;
        }

        let decoded = match decoder.decode(&packet) {
            Ok(d) => d,
            Err(symphonia::core::errors::Error::IoError(_))
            | Err(symphonia::core::errors::Error::DecodeError(_)) => continue,
            Err(err) => return Err(AppError::Transcription(format!("Decode error: {err}"))),
        };

        // Determine actual sample rate and channels from first decoded frame
        if !first_frame_decoded {
            actual_native_rate = decoded.spec().rate;
            actual_num_channels = decoded.spec().channels.count();
            first_frame_decoded = true;
            resampler = create_resampler(actual_native_rate)?;
        }

        let mono = decode_to_mono_f32(&decoded, actual_num_channels);

        let pcm = if let Some(ref resamp) = resampler {
            resamp.resample(&mono, false)
        } else {
            mono
        };

        let chunk_start = total_16k_samples;
        let chunk_end = total_16k_samples + pcm.len();
        total_16k_samples = chunk_end;

        if chunk_end <= start_sample {
            continue;
        }

        let copy_start = start_sample.saturating_sub(chunk_start);
        let copy_end = (end_sample - chunk_start).min(pcm.len());

        if copy_start < copy_end {
            result.extend_from_slice(&pcm[copy_start..copy_end]);
        }
    }

    if result.is_empty() {
        return Err(AppError::Transcription(
            "No audio data in the specified time range".into(),
        ));
    }

    Ok(result)
}
