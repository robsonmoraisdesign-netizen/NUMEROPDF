use eframe::egui;
use numeropdf::Pdf;
use pdfcraft_render::{RenderConfig, RenderPool, RenderRequest};
use std::{
    collections::VecDeque,
    sync::Arc,
    time::{Duration, Instant},
};
type Key = (usize, u32);
#[derive(Default)]
pub struct Viewer {
    source: Option<Arc<Vec<u8>>>,
    pool: Option<RenderPool>,
    wanted: Option<Key>,
    submitted: Option<Key>,
    changed: Option<Instant>,
    tag: u64,
    waiting: bool,
    texture: Option<egui::TextureHandle>,
    cache: VecDeque<(Key, egui::TextureHandle)>,
}
impl Viewer {
    pub fn texture(&self) -> Option<&egui::TextureHandle> {
        self.texture.as_ref()
    }
    pub fn update(
        &mut self,
        ctx: &egui::Context,
        pdf: &Pdf,
        page: usize,
        scale: f32,
    ) -> Option<String> {
        if !self
            .source
            .as_ref()
            .is_some_and(|b| Arc::ptr_eq(b, &pdf.bytes))
        {
            *self = Self::default();
            self.source = Some(pdf.bytes.clone());
            self.pool = Some(RenderPool::new(
                pdf.bytes.clone(),
                1,
                RenderConfig::default(),
            ));
        }
        let info = &pdf.pages[page];
        let limit = (64.0 * 1024.0 * 1024.0 / 4.0 / (info.width * info.height)).sqrt();
        let key = (page, (scale.min(limit) * 64.0).floor().max(1.0) as u32);
        if self.wanted != Some(key) {
            if self.wanted.is_none_or(|k| k.0 != page) {
                self.texture = None;
            }
            self.wanted = Some(key);
            self.changed = Some(Instant::now());
            self.tag += 1;
            self.submitted = None;
            self.waiting = false;
            self.pool.as_ref().unwrap().set_queue(vec![]);
            if let Some((_, texture)) = self.cache.iter().find(|(k, _)| *k == key) {
                self.texture = Some(texture.clone());
                self.submitted = Some(key);
            }
        }
        let pool = self.pool.as_ref().unwrap();
        let mut error = None;
        while let Some(out) = pool.try_recv() {
            if out.request.tag != self.tag {
                continue;
            }
            self.waiting = false;
            if let Some(e) = out.error {
                error = Some(e);
                continue;
            }
            if !out.warnings.is_empty() {
                error = Some(format!("Renderização incompleta: {:?}", out.warnings));
                continue;
            }
            let texture = ctx.load_texture(
                "pagina",
                egui::ColorImage::from_rgba_premultiplied(
                    [out.width as usize, out.height as usize],
                    &out.rgba,
                ),
                egui::TextureOptions::LINEAR,
            );
            self.cache.push_back((key, texture.clone()));
            while self
                .cache
                .iter()
                .map(|(_, t)| t.size()[0] * t.size()[1] * 4)
                .sum::<usize>()
                > 64 * 1024 * 1024
                && self.cache.len() > 1
            {
                self.cache.pop_front();
            }
            self.texture = Some(texture);
        }
        if self.submitted != Some(key)
            && (self.texture.is_none()
                || self.changed.unwrap().elapsed() >= Duration::from_millis(150))
        {
            pool.set_queue(vec![RenderRequest {
                page,
                scale: key.1 as f32 / 64.0,
                tag: self.tag,
                ..Default::default()
            }]);
            self.submitted = Some(key);
            self.waiting = true;
        }
        // Poll independently of edit/export operations; repainting never requeues a completed render.
        if self.waiting || self.submitted != Some(key) {
            ctx.request_repaint_after(Duration::from_millis(50));
        }
        error
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn redraws_reuse_parser_and_cached_texture() {
        let mut doc = pdfcraft_cos::Document::new_empty();
        pdfcraft_organize::insert_blank_page(&mut doc, 0, 200.0, 200.0).unwrap();
        let pdf =
            Pdf::from_bytes(pdfcraft_cos::write_full(&doc, &Default::default()).unwrap()).unwrap();
        let ctx = egui::Context::default();
        let mut viewer = Viewer::default();
        let settle = |v: &mut Viewer, scale| {
            let deadline = Instant::now() + Duration::from_secs(15);
            loop {
                assert!(v.update(&ctx, &pdf, 0, scale).is_none());
                if v.texture.is_some() && !v.waiting && v.submitted == v.wanted {
                    break;
                }
                assert!(Instant::now() < deadline, "render timed out");
                std::thread::sleep(Duration::from_millis(10));
            }
        };
        settle(&mut viewer, 1.0);
        let stats = viewer.pool.as_ref().unwrap().stats();
        for _ in 0..20 {
            viewer.update(&ctx, &pdf, 0, 1.0);
        }
        assert_eq!(
            stats.rasterizations,
            viewer.pool.as_ref().unwrap().stats().rasterizations
        );
        viewer.update(&ctx, &pdf, 0, 1.5);
        assert!(viewer.texture.is_some());
        settle(&mut viewer, 1.5);
        assert_eq!(
            stats.parser_builds,
            viewer.pool.as_ref().unwrap().stats().parser_builds
        );
        let rendered = viewer.pool.as_ref().unwrap().stats().rasterizations;
        settle(&mut viewer, 1.0);
        assert_eq!(
            rendered,
            viewer.pool.as_ref().unwrap().stats().rasterizations
        );
    }
}
