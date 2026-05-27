use clap::Parser;
use serde::Serialize;
use std::fmt::Display;
use std::fs::File;
use std::path::PathBuf;
use std::time::Duration;

#[derive(Serialize)]
pub enum TestKind {
    Sine { freq: f32, amp: f32 },
    Square { freq: f32, amp: f32 },
    Sawtooth { freq: f32, amp: f32 },
    Complex { abs_path: String },
}

impl Display for TestKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TestKind::Sine { freq, amp: _ } => write!(f, "sine_{:.0}Hz", freq),
            TestKind::Square { freq, amp: _ } => write!(f, "square_{:.0}Hz", freq),
            TestKind::Sawtooth { freq, amp: _ } => write!(f, "sawtooth_{:.0}Hz", freq),
            TestKind::Complex { abs_path: _ } => write!(f, "complex"),
        }
    }
}

#[derive(Serialize)]
pub enum SampleType {
    F32,
    I16,
}

impl Display for SampleType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SampleType::F32 => write!(f, "f32"),
            SampleType::I16 => write!(f, "i16"),
        }
    }
}

#[derive(Serialize)]
pub struct Params {
    sample_type: SampleType,
    sample_rate: u32,
    duration: Duration,
    kind: TestKind,
}

#[derive(Debug, Parser)]
struct Arguments {
    #[clap(short, long)]
    output: String,
}

fn interleaved_sin(params: &Params, freq: f32, amp: f32) -> Vec<f32> {
    let num_samples = (params.sample_rate as f32 * params.duration.as_secs_f32()) as usize;
    let mut samples = Vec::with_capacity(num_samples * 2); // stereo

    for i in 0..num_samples {
        let t = i as f32 / params.sample_rate as f32;
        let sample_value = amp * (2.0 * std::f32::consts::PI * freq * t).sin();
        samples.push(sample_value); // left channel
        samples.push(sample_value); // right channel
    }

    samples
}

fn interleaved_square(params: &Params, freq: f32, amp: f32) -> Vec<f32> {
    let num_samples = (params.sample_rate as f32 * params.duration.as_secs_f32()) as usize;
    let mut samples = Vec::with_capacity(num_samples * 2);

    for i in 0..num_samples {
        let t = i as f32 / params.sample_rate as f32;
        let sample_value = if (2.0 * std::f32::consts::PI * freq * t).sin() >= 0.0 {
            amp
        } else {
            -amp
        };
        samples.push(sample_value); // left channel
        samples.push(sample_value); // right channel
    }

    samples
}

fn interleaved_sawtooth(params: &Params, freq: f32, amp: f32) -> Vec<f32> {
    let num_samples = (params.sample_rate as f32 * params.duration.as_secs_f32()) as usize;
    let mut samples = Vec::with_capacity(num_samples * 2);

    for i in 0..num_samples {
        let t = i as f32 / params.sample_rate as f32;
        let sample_value = amp * (2.0 * (freq * t - (freq * t).floor()) - 1.0);
        samples.push(sample_value); // left channel
        samples.push(sample_value); // right channel
    }

    samples
}

fn save_test(out_dir: &str, params: &Params) -> Result<(), Box<dyn std::error::Error>> {
    let mut path = PathBuf::from(out_dir);
    path.push(format!(
        "{}_{}Hz_{}s_{}.wav",
        params.kind,
        params.sample_rate,
        params.duration.as_secs(),
        params.sample_type
    ));

    let spec = hound::WavSpec {
        channels: 2,
        sample_rate: params.sample_rate,
        bits_per_sample: match params.sample_type {
            SampleType::F32 => 32,
            SampleType::I16 => 16,
        },
        sample_format: match params.sample_type {
            SampleType::F32 => hound::SampleFormat::Float,
            SampleType::I16 => hound::SampleFormat::Int,
        },
    };

    /* Generate samples */
    let samples = match params.kind {
        TestKind::Sine { freq, amp } => interleaved_sin(params, freq, amp),
        TestKind::Square { freq, amp } => interleaved_square(params, freq, amp),
        TestKind::Sawtooth { freq, amp } => interleaved_sawtooth(params, freq, amp),
        TestKind::Complex { .. } => {
            unimplemented!()
        }
    };

    /* Write WAV file */
    let mut writter = hound::WavWriter::create(path.clone(), spec)?;
    match params.sample_type {
        SampleType::F32 => {
            for sample in samples {
                writter.write_sample(sample)?;
            }
        }
        SampleType::I16 => {
            for sample in samples {
                let int_sample = (sample.clamp(-1.0, 1.0) * i16::MAX as f32).round() as i16;
                writter.write_sample(int_sample)?;
            }
        }
    }
    writter.finalize()?;

    /* Write metadata json file */
    let json_path = path.with_extension("json");
    serde_json::to_writer_pretty(File::create(&json_path)?, &params)?;

    /* Convert .WAV into .mp3 using ffmpeg */
    let mp3_path = path.with_extension("mp3");
    let output = std::process::Command::new("ffmpeg")
        .args([
            "-y",
            "-hide_banner",
            "-loglevel",
            "error",
            "-i",
            path.to_str().unwrap(),
            "-codec:a",
            "libmp3lame",
            "-q:a",
            "2",
            "-metadata", "artist=gentests",
            "-metadata", "album=generated",
            mp3_path.to_str().unwrap(),
        ])
        .output()?;

    if !output.status.success() {
        return Err(format!(
            "ffmpeg failed:\n{}",
            String::from_utf8_lossy(&output.stderr)
        )
        .into());
    }

    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Arguments::parse();

    for freq in [110.0, 880.0, 3520.0, 7040.0] {
        for kind in [
            TestKind::Sine { freq, amp: 0.8 },
            TestKind::Square { freq, amp: 0.8 },
            TestKind::Sawtooth { freq, amp: 0.8 },
        ] {
            save_test(
                &args.output,
                &Params {
                    sample_type: SampleType::I16,
                    sample_rate: 44100,
                    duration: Duration::from_secs(2),
                    kind,
                },
            )?;
        }
    }

    Ok(())
}
