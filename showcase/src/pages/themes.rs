//! `Themes` page — placeholder until its widget-group task lands.

use tuiforge::prelude::*;

use super::{Ctx, Page};

#[derive(Default)]
pub struct ThemesPage;

impl Page for ThemesPage {
    fn title(&self) -> &'static str {
        "Themes"
    }
    fn icon(&self) -> &'static str {
        "◑"
    }
    fn draw(&mut self, area: Rect, buf: &mut Buffer, ctx: &mut Ctx) {
        let th = &ctx.theme;
        put_centered(buf, Rect { y: area.y + area.height / 2, height: 1, ..area }, "coming soon", st(th.text_muted, th.background));
    }
    fn event(&mut self, _ev: &Event, _ctx: &mut Ctx) -> Outcome {
        Outcome::Ignored
    }
}
