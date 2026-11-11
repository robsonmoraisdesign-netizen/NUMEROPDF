use crate::{Numbering, Pdf};
use std::path::{Path, PathBuf};
#[derive(Debug)]
pub struct Outcome {
    pub input: PathBuf,
    pub output: Option<PathBuf>,
    pub error: Option<String>,
}
/// Sequential processing bounds memory; each document restarts the configured sequence.
pub fn number_files(
    inputs: &[PathBuf],
    directory: &Path,
    settings: &Numbering,
    to_end: bool,
    mut progress: impl FnMut(usize, usize),
) -> Vec<Outcome> {
    inputs
        .iter()
        .enumerate()
        .map(|(i, input)| {
            let result = (|| {
                let pdf = Pdf::open(input)?;
                let mut config = settings.clone();
                if to_end {
                    config.last_page = pdf.pages.len() - 1;
                }
                let output = directory.join(format!(
                    "{:04}-{}-numerado.pdf",
                    i + 1,
                    input.file_stem().unwrap_or_default().to_string_lossy()
                ));
                pdf.numbered(&config)?.save(&output)?;
                Ok::<_, crate::Error>(output)
            })();
            progress(i + 1, inputs.len());
            match result {
                Ok(output) => Outcome {
                    input: input.clone(),
                    output: Some(output),
                    error: None,
                },
                Err(e) => Outcome {
                    input: input.clone(),
                    output: None,
                    error: Some(e.to_string()),
                },
            }
        })
        .collect()
}
