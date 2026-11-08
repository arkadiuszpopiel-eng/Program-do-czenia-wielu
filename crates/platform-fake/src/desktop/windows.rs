//! `DesktopPort` wirtualnego pulpitu: lista okien, fokus, położenie i stan — przez strażnika celów.

use platform_contract::{
    DesktopPort, DesktopWindow, GuiError, MonitorInfo, ScreenRect, TargetGuard, WindowId,
    WindowState, validate_bounds,
};

use super::{FakeDesktop, GuiRecordKind};

impl DesktopPort for FakeDesktop {
    fn guard(&self) -> &TargetGuard {
        &self.guard
    }

    fn windows(&self) -> Result<Vec<DesktopWindow>, GuiError> {
        let s = self.lock();
        Ok(s.windows.iter().map(|w| self.described(&s, w)).collect())
    }

    fn foreground(&self) -> Result<Option<DesktopWindow>, GuiError> {
        let s = self.lock();
        Ok(s.windows
            .iter()
            .find(|w| w.info.focused)
            .map(|w| self.described(&s, w)))
    }

    fn window_at(&self, x: i32, y: i32) -> Result<Option<WindowId>, GuiError> {
        Ok(self.lock().top_at(x, y).map(|w| w.info.id))
    }

    fn monitors(&self) -> Result<Vec<MonitorInfo>, GuiError> {
        Ok(self.lock().monitors.clone())
    }

    fn focus(&self, id: WindowId) -> Result<(), GuiError> {
        let mut s = self.lock();
        let w = s.win(id)?;
        self.check_win(&s, w, "fokus okna")?;
        s.raise(id);
        s.record(id, GuiRecordKind::Window("focus".into()));
        Ok(())
    }

    fn set_bounds(&self, id: WindowId, rect: ScreenRect) -> Result<(), GuiError> {
        let mut s = self.lock();
        let w = s.win(id)?;
        self.check_win(&s, w, "zmiana położenia okna")?;
        validate_bounds(&rect, &s.monitors)?;
        let w = s.win_mut(id)?;
        w.info.state = WindowState::Normal;
        w.info.rect = rect;
        w.restore = rect;
        s.record(id, GuiRecordKind::Window("bounds".into()));
        Ok(())
    }

    fn set_state(&self, id: WindowId, state: WindowState) -> Result<(), GuiError> {
        let mut s = self.lock();
        let w = s.win(id)?;
        self.check_win(&s, w, "stan okna")?;
        let work = s.monitors.first().map(|m| m.work_area).unwrap_or_default();
        let focused = s.windows.iter().find(|w| w.info.focused).map(|w| w.info.id);
        let w = s.win_mut(id)?;
        match state {
            WindowState::Maximized => w.info.rect = work,
            WindowState::Normal => w.info.rect = w.restore,
            WindowState::Minimized => {}
        }
        w.info.state = state;
        if state == WindowState::Minimized {
            s.renumber(focused.filter(|f| *f != id));
        } else {
            s.raise(id);
        }
        s.record(id, GuiRecordKind::Window(format!("state:{state:?}")));
        Ok(())
    }
}
