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
                let output = directory.join(
                    input
                        .file_name()
                        .ok_or_else(|| crate::Error("Nome de arquivo inválido".into()))?,
                );
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

use std::sync::Arc;

pub struct NumberInput {
    pub input: PathBuf,
    pub pdf: Arc<Pdf>,
    pub config: Numbering,
}
pub struct Generated {
    pub input: PathBuf,
    pub config: Numbering,
    pub result: std::result::Result<Arc<Pdf>, String>,
}
pub struct ExportInput {
    pub input: PathBuf,
    pub result: std::result::Result<Arc<Pdf>, String>,
}
/// Generate from the already imported source, never from an older numbering preview.
pub fn generate_all(
    inputs: Vec<NumberInput>,
    mut progress: impl FnMut(usize, usize),
) -> Vec<Generated> {
    let total = inputs.len();
    inputs
        .into_iter()
        .enumerate()
        .map(|(i, item)| {
            let result = item
                .pdf
                .numbered(&item.config)
                .map(Arc::new)
                .map_err(|e| e.to_string());
            progress(i + 1, total);
            Generated {
                input: item.input,
                config: item.config,
                result,
            }
        })
        .collect()
}
/// Preserve exact original basenames. Duplicate names are reported, never renamed or overwritten.
pub fn export_all(
    inputs: Vec<ExportInput>,
    directory: &Path,
    mut progress: impl FnMut(usize, usize),
) -> Vec<Outcome> {
    let mut names = std::collections::HashMap::<String, usize>::new();
    for item in &inputs {
        let key = item
            .input
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_lowercase();
        *names.entry(key).or_default() += 1;
    }
    let total = inputs.len();
    inputs
        .into_iter()
        .enumerate()
        .map(|(i, item)| {
            let result = (|| {
                let name = item
                    .input
                    .file_name()
                    .ok_or_else(|| "Nome de arquivo inválido".to_string())?;
                if names[&name.to_string_lossy().to_lowercase()] > 1 {
                    return Err(
                        "Há arquivos com o mesmo nome no lote. Exporte-os para pastas diferentes."
                            .into(),
                    );
                }
                let pdf = item.result?;
                let output = directory.join(name);
                pdf.save(&output).map_err(|e| {
                    format!("Não foi possível salvar sem sobrescrever arquivo existente: {e}")
                })?;
                Ok(output)
            })();
            progress(i + 1, total);
            match result {
                Ok(output) => Outcome {
                    input: item.input,
                    output: Some(output),
                    error: None,
                },
                Err(error) => Outcome {
                    input: item.input,
                    output: None,
                    error: Some(error),
                },
            }
        })
        .collect()
}
