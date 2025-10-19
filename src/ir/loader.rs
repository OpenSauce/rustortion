use anyhow::{Context, Result, anyhow};
use hound::WavReader;
use log::{debug, info, warn};
use std::fs;
use std::path::{Path, PathBuf};

pub struct IrLoader {
    available_ir_paths: Vec<(String, PathBuf)>,
    ir_directory: PathBuf,
    target_sample_rate: u32,
}

impl IrLoader {
    pub fn new(directory: &Path, target_sample_rate: u32) -> Result<IrLoader> {
        let mut loader = IrLoader {
            available_ir_paths: Vec::new(),
            ir_directory: directory.to_path_buf(),
            target_sample_rate,
        };

        loader.scan_ir_directory()?;

        Ok(loader)
    }

    pub fn get_first(&self) -> Result<Vec<f32>> {
        if self.available_ir_paths.is_empty() {
            return Err(anyhow!("available_ir_paths is empty"));
        }

        self.load_ir(&self.available_ir_paths[0].1)
    }

    pub fn load_by_name(&self, name: &str) -> Result<Vec<f32>> {
        for (ir_name, ir_path) in &self.available_ir_paths {
            if ir_name == name {
                return self.load_ir(ir_path);
            }
        }

        Err(anyhow!("ir name '{}' not found", name))
    }

    pub fn load_ir(&self, path: &Path) -> Result<Vec<f32>> {
        let reader = WavReader::open(path).context("Failed to open WAV file")?;
        let spec = reader.spec();

        let samples: Vec<f32> = if spec.sample_format == hound::SampleFormat::Float {
            reader
                .into_samples::<f32>()
                .collect::<Result<Vec<_>, _>>()
                .context("Failed to read float samples")?
        } else {
            let max_val = (1 << (spec.bits_per_sample - 1)) as f32;
            reader
                .into_samples::<i32>()
                .map(|s| s.map(|v| v as f32 / max_val))
                .collect::<Result<Vec<_>, _>>()
                .context("Failed to read integer samples")?
        };

        let mono = if spec.channels > 1 {
            samples
                .chunks(spec.channels as usize)
                .map(|c| c.iter().sum::<f32>() / spec.channels as f32)
                .collect()
        } else {
            samples
        };

        let mut resampled = if spec.sample_rate != self.target_sample_rate {
            debug!(
                "Resampling IR from {} Hz to {} Hz",
                spec.sample_rate, self.target_sample_rate
            );
            resample_linear(&mono, spec.sample_rate, self.target_sample_rate)
        } else {
            mono
        };

        if let Some(max) = resampled.iter().fold(None::<f32>, |m, &x| {
            Some(m.map_or(x.abs(), |mm| mm.max(x.abs())))
        }) && max > 0.0
        {
            let g = 0.9 / max;
            for s in &mut resampled {
                *s *= g;
            }
        }

        Ok(resampled)
    }

    pub fn scan_ir_directory(&mut self) -> Result<()> {
        if !self.ir_directory.exists() {
            fs::create_dir_all(&self.ir_directory).context("Failed to create IR directory")?;
            warn!("IR directory created at {:?}", self.ir_directory);
            return Ok(());
        }

        self.available_ir_paths.clear();
        let base = self.ir_directory.clone();
        self.scan_recursive(&base, &base)?;

        self.available_ir_paths.sort_by(|a, b| {
            let a_sep_count = a.0.matches('/').count();
            let b_sep_count = b.0.matches('/').count();
            a_sep_count.cmp(&b_sep_count).then_with(|| a.0.cmp(&b.0))
        });

        info!(
            "Found {} impulse response files",
            self.available_ir_paths.len()
        );
        Ok(())
    }

    fn scan_recursive(&mut self, current_dir: &Path, base_dir: &Path) -> Result<()> {
        for entry in fs::read_dir(current_dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.is_dir() {
                self.scan_recursive(&path, base_dir)?;
            } else if path.extension().and_then(|s| s.to_str()) == Some("wav") {
                let relative_path = path
                    .strip_prefix(base_dir)
                    .unwrap_or(&path)
                    .to_string_lossy()
                    .replace('\\', "/");

                self.available_ir_paths.push((relative_path, path));
            }
        }
        Ok(())
    }
}

fn resample_linear(samples: &[f32], from_rate: u32, to_rate: u32) -> Vec<f32> {
    let ratio = from_rate as f64 / to_rate as f64;
    let new_len = (samples.len() as f64 / ratio) as usize;
    let mut out = Vec::with_capacity(new_len);

    for i in 0..new_len {
        let src_pos = i as f64 * ratio;
        let src_idx = src_pos as usize;
        let frac = src_pos - src_idx as f64;

        let s = if src_idx + 1 < samples.len() {
            samples[src_idx] * (1.0 - frac as f32) + samples[src_idx + 1] * frac as f32
        } else if src_idx < samples.len() {
            samples[src_idx]
        } else {
            0.0
        };
        out.push(s);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_scan_ir_directory_finds_wavs() -> anyhow::Result<()> {
        let tmp = TempDir::new()?;
        let ir_dir = tmp.path().join("irs");
        std::fs::create_dir_all(ir_dir.join("nested"))?;

        std::fs::write(ir_dir.join("a.wav"), "")?;
        std::fs::write(ir_dir.join("nested").join("b.wav"), "")?;

        let mut cab = IrLoader::new(&ir_dir, 48000)?;
        cab.scan_ir_directory()?;

        let names = cab
            .available_ir_paths
            .iter()
            .map(|(name, _)| name.as_str())
            .collect::<Vec<&str>>();
        assert_eq!(names, vec!["a.wav", "nested/b.wav"]);

        Ok(())
    }
}
