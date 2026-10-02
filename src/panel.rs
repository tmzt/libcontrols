//! Structured container panel with background styling, metallic borders, headers, and active
//! states.

use std::cell::RefCell;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};

use libmsdf::drawlist::{DrawList, LINE_BOX_RATIO, SdfInstance, SdfKind};

use crate::event::{
    ControlAction, ControlPart, EventResponse, InputEvent, Key, KeyboardEvent, MouseButton,
    MouseEvent,
};
use crate::handle::{ControlHandle, ReadValue};
use crate::host::{Control, ControlHost, ControlKey, DragGrab, HostedControl};
use crate::rect::Rect;
use crate::ring::{GlowRingStyle, draw_glowing_ring};
use crate::traits::{DrawContext, DrawControl};

/// Mutable runtime state for a container panel.
#[derive(Debug, Default)]
pub struct PanelState {
    /// Optional title text.
    pub title: Mutex<Option<String>>,
    /// Optional subtitle text.
    pub subtitle: Mutex<Option<String>>,
    /// Whether this panel is in an active/selected state.
    pub is_active: AtomicBool,
}

impl PanelState {
    /// Create a new panel state.
    pub fn new(title: Option<impl Into<String>>, subtitle: Option<impl Into<String>>) -> Self {
        Self {
            title: Mutex::new(title.map(Into::into)),
            subtitle: Mutex::new(subtitle.map(Into::into)),
            is_active: AtomicBool::new(false),
        }
    }

    /// Read panel title.
    pub fn title(&self) -> Option<String> {
        self.title.lock().unwrap().clone()
    }

    /// Set panel title.
    pub fn set_title(&self, title: Option<impl Into<String>>) {
        *self.title.lock().unwrap() = title.map(Into::into);
    }

    /// Read panel subtitle.
    pub fn subtitle(&self) -> Option<String> {
        self.subtitle.lock().unwrap().clone()
    }

    /// Set panel subtitle.
    pub fn set_subtitle(&self, subtitle: Option<impl Into<String>>) {
        *self.subtitle.lock().unwrap() = subtitle.map(Into::into);
    }

    /// Check if panel is active.
    #[inline]
    pub fn is_active(&self) -> bool {
        self.is_active.load(Ordering::Relaxed)
    }

    /// Set active state.
    #[inline]
    pub fn set_active(&self, active: bool) {
        self.is_active.store(active, Ordering::Relaxed);
    }
}

/// Application data a panel carries alongside its child controls.
#[derive(Debug, Default)]
pub struct PanelData<S> {
    inner: Mutex<S>,
}

impl<S> PanelData<S> {
    /// Wrap `value` for sharing with a panel.
    pub fn new(value: S) -> Arc<Self> {
        Arc::new(Self {
            inner: Mutex::new(value),
        })
    }

    /// Read the data under the lock.
    pub fn read<R>(&self, reader: impl FnOnce(&S) -> R) -> R {
        reader(&self.inner.lock().unwrap())
    }

    /// Mutate the data under the lock.
    pub fn write<R>(&self, writer: impl FnOnce(&mut S) -> R) -> R {
        writer(&mut self.inner.lock().unwrap())
    }

    /// Replace the data outright.
    pub fn set(&self, value: S) {
        *self.inner.lock().unwrap() = value;
    }
}

impl<S: Clone> PanelData<S> {
    /// Clone the data out.
    pub fn get(&self) -> S {
        self.inner.lock().unwrap().clone()
    }
}

/// Container panel control with optional background, metallic border, header title, and
/// subtitle.
pub struct Panel<'a, S = ()> {
    /// Shared mutable panel state.
    pub state: Arc<PanelState>,
    /// Outer bounding box.
    pub rect: Rect,
    /// Corner radius for rounded box and border.
    pub corner_radius: Option<f32>,
    /// Border stroke thickness.
    pub border_thickness: Option<f32>,
    /// Border stroke color.
    pub border_color: Option<[f32; 4]>,
    /// Optional background fill color (`None` for transparent / pass-through viewports).
    pub bg_color: Option<[f32; 4]>,
    /// Header title text color.
    pub title_color: Option<[f32; 4]>,
    /// Header subtitle text color.
    pub subtitle_color: Option<[f32; 4]>,
    /// Header title font size.
    pub title_font_size: Option<f32>,
    /// Header subtitle font size.
    pub subtitle_font_size: Option<f32>,
    /// Internal padding for contents/header.
    pub padding: f32,
    /// Custom glowing ring style.
    pub glow_style: Option<GlowRingStyle>,
    children: RefCell<ControlHost<'a>>,
    data: Option<Arc<PanelData<S>>>,
}

impl<'a> Panel<'a, ()> {
    /// Create a new panel at the given bounds, carrying no data.
    pub fn new(rect: Rect) -> Self {
        Self {
            state: Arc::new(PanelState::default()),
            rect,
            corner_radius: None,
            border_thickness: None,
            border_color: None,
            bg_color: None,
            title_color: None,
            subtitle_color: None,
            title_font_size: None,
            subtitle_font_size: None,
            padding: 10.0,
            glow_style: None,
            children: RefCell::new(ControlHost::builder().build()),
            data: None,
        }
    }

    /// Attach the data this panel's controls edit.
    pub fn with_data<S>(self, data: Arc<PanelData<S>>) -> Panel<'a, S> {
        Panel {
            state: self.state,
            rect: self.rect,
            corner_radius: self.corner_radius,
            border_thickness: self.border_thickness,
            border_color: self.border_color,
            bg_color: self.bg_color,
            title_color: self.title_color,
            subtitle_color: self.subtitle_color,
            title_font_size: self.title_font_size,
            subtitle_font_size: self.subtitle_font_size,
            padding: self.padding,
            glow_style: self.glow_style,
            children: self.children,
            data: Some(data),
        }
    }
}

impl<'a, S> Panel<'a, S> {
    /// The data this panel carries, if any.
    pub fn data(&self) -> Option<&Arc<PanelData<S>>> {
        self.data.as_ref()
    }

    /// Read the carried data, returning `None` when the panel carries none.
    pub fn read_data<R>(&self, reader: impl FnOnce(&S) -> R) -> Option<R> {
        self.data.as_ref().map(|data| data.read(reader))
    }

    /// Mutate the carried data, returning `None` when the panel carries none.
    pub fn write_data<R>(&self, writer: impl FnOnce(&mut S) -> R) -> Option<R> {
        self.data.as_ref().map(|data| data.write(writer))
    }

    /// Attach a shared `PanelState`.
    pub fn with_state(mut self, state: Arc<PanelState>) -> Self {
        self.state = state;
        self
    }

    /// Set title text.
    pub fn with_title(self, title: impl Into<String>) -> Self {
        self.state.set_title(Some(title));
        self
    }

    /// Set subtitle text.
    pub fn with_subtitle(self, subtitle: impl Into<String>) -> Self {
        self.state.set_subtitle(Some(subtitle));
        self
    }

    /// Set corner radius.
    pub fn with_corner_radius(mut self, radius: f32) -> Self {
        self.corner_radius = Some(radius);
        self
    }

    /// Set border stroke color and thickness.
    pub fn with_border(mut self, color: [f32; 4], thickness: f32) -> Self {
        self.border_color = Some(color);
        self.border_thickness = Some(thickness);
        self
    }

    /// Set background fill color (`None` for pass-through 3D viewports).
    pub fn with_bg(mut self, bg_color: [f32; 4]) -> Self {
        self.bg_color = Some(bg_color);
        self
    }

    /// Set header title color.
    pub fn with_title_color(mut self, color: [f32; 4]) -> Self {
        self.title_color = Some(color);
        self
    }

    /// Set header subtitle color.
    pub fn with_subtitle_color(mut self, color: [f32; 4]) -> Self {
        self.subtitle_color = Some(color);
        self
    }

    /// Set header title font size.
    pub fn with_title_font_size(mut self, font_size: f32) -> Self {
        self.title_font_size = Some(font_size);
        self
    }

    /// Set header subtitle font size.
    pub fn with_subtitle_font_size(mut self, font_size: f32) -> Self {
        self.subtitle_font_size = Some(font_size);
        self
    }

    /// Set internal padding.
    pub fn with_padding(mut self, padding: f32) -> Self {
        self.padding = padding;
        self
    }

    /// Set active state (glow ring).
    pub fn with_active(self, active: bool) -> Self {
        self.state.set_active(active);
        self
    }

    /// Set custom glowing ring style.
    pub fn with_glow_style(mut self, style: GlowRingStyle) -> Self {
        self.glow_style = Some(style);
        self
    }

    /// Read title text.
    pub fn title(&self) -> Option<String> {
        self.state.title()
    }

    /// Read subtitle text.
    pub fn subtitle(&self) -> Option<String> {
        self.state.subtitle()
    }

    /// Check if active.
    pub fn is_active(&self) -> bool {
        self.state.is_active()
    }

    /// Calculate inner content rectangle below any headers.
    pub fn content_rect(&self, font_size_base: f32) -> Rect {
        let mut top_offset = self.padding;
        if self.title().is_some() {
            let t_sz = self.title_font_size.unwrap_or(font_size_base * 1.1);
            top_offset += t_sz * LINE_BOX_RATIO + 4.0;
        }
        if self.subtitle().is_some() {
            let st_sz = self.subtitle_font_size.unwrap_or(font_size_base * 0.85);
            top_offset += st_sz * LINE_BOX_RATIO + 4.0;
        }
        Rect::new(
            self.rect.x + self.padding,
            self.rect.y + top_offset,
            (self.rect.w - self.padding * 2.0).max(0.0),
            (self.rect.h - top_offset - self.padding).max(0.0),
        )
    }

    /// Obtain a borrowed handle to inspect the panel.
    pub fn handle(&self) -> ControlHandle<'_, Self> {
        ControlHandle::new(self)
    }

    /// Add an unnamed child control to this panel.
    pub fn with_control<C: Control + 'a>(mut self, control: ControlHandle<'a, C>) -> Self {
        self.children.get_mut().add_control(control);
        self
    }

    /// Add a named child control to this panel.
    pub fn with_named_control<C: Control + 'a>(
        mut self,
        id: impl Into<String>,
        control: ControlHandle<'a, C>,
    ) -> Self {
        self.children.get_mut().add_named_control(id, control);
        self
    }

    /// Deliver an event to the panel's child controls.
    pub fn deliver_event(&self, event: InputEvent) -> EventResponse {
        self.children.borrow_mut().deliver_event(event)
    }

    /// Put a drag back in progress, from before this panel was built.
    #[must_use]
    pub fn with_active_drag(self, grab: Option<DragGrab>) -> Self {
        self.children.borrow_mut().set_active_drag(grab);
        self
    }

    /// The drag in progress, to be carried to the next build.
    #[must_use]
    pub fn active_drag(&self) -> Option<DragGrab> {
        self.children.borrow().active_drag()
    }

    /// Set the initial focus within this panel's child host.
    #[must_use]
    pub fn with_focus(mut self, focused: Option<usize>) -> Self {
        self.children.get_mut().set_focused_index(focused);
        self
    }

    /// Set the initial focus by index.
    #[must_use]
    pub fn with_initial_focus(self, index: usize) -> Self {
        self.with_focus(Some(index))
    }

    /// Put the focus back on the control it was on before this rebuild.
    #[must_use]
    pub fn with_focus_key(mut self, key: Option<ControlKey>) -> Self {
        self.children.get_mut().set_focused_key(key);
        self
    }

    /// The focused control as a key, to be carried to the next build.
    #[must_use]
    pub fn focused_key(&self) -> Option<ControlKey> {
        self.children.borrow().focused_key()
    }

    /// Where the focused control sits in this panel, if it is still here.
    #[must_use]
    pub fn focused_index(&self) -> Option<usize> {
        self.children.borrow().focused_index()
    }
}

impl<S> DrawControl for Panel<'_, S> {
    fn draw(&self, list: &mut DrawList, ctx: &DrawContext) {
        let rad = self.corner_radius.unwrap_or(ctx.theme.corner_radius);

        if let Some(bg_col) = self.bg_color {
            let kind = if rad > 0.0 {
                SdfKind::RoundedBox { radius: rad }
            } else {
                SdfKind::Box
            };
            list.push(SdfInstance {
                kind,
                position: self.rect.pos(),
                size: self.rect.size(),
                color: bg_col,
                anim: 0,
            });
        }

        if let Some(border_col) = self.border_color {
            let thick = self.border_thickness.unwrap_or(1.0);
            if thick > 0.0 {
                list.push(SdfInstance {
                    kind: SdfKind::Outline {
                        radius: rad,
                        thickness: thick,
                    },
                    position: self.rect.pos(),
                    size: self.rect.size(),
                    color: border_col,
                    anim: 0,
                });
            }
        }

        let mut text_y = self.rect.y + self.padding;
        if let Some(ref title) = self.title() {
            let t_sz = self
                .title_font_size
                .unwrap_or(ctx.theme.default_font_size * 1.1);
            let t_col = self.title_color.unwrap_or([0.25, 0.88, 1.0, 1.0]);
            let t_x = self.rect.x + self.padding;
            ctx.draw_text(list, title, [t_x, text_y], t_sz, t_col);
            text_y += t_sz * LINE_BOX_RATIO + 4.0;
        }

        if let Some(ref subtitle) = self.subtitle() {
            let st_sz = self
                .subtitle_font_size
                .unwrap_or(ctx.theme.default_font_size * 0.85);
            let st_col = self.subtitle_color.unwrap_or([0.65, 0.78, 0.90, 0.90]);
            let st_x = self.rect.x + self.padding;
            ctx.draw_text(list, subtitle, [st_x, text_y], st_sz, st_col);
        }

        if self.is_active() {
            let style = self.glow_style.unwrap_or_default();
            draw_glowing_ring(list, self.rect, &style, &ctx.theme);
        }
        self.children.borrow().draw(list, ctx);
    }
}

impl<S> HostedControl for Panel<'_, S> {
    fn bounds(&self) -> Rect {
        self.rect
    }

    fn hit_test(&self, pos: [f32; 2]) -> Option<(ControlPart, f32)> {
        if self.rect.contains(pos) {
            Some((ControlPart::Body, self.rect.normalized_x(pos[0])))
        } else {
            None
        }
    }

    fn handle_mouse(&self, event: &MouseEvent, hit_part: ControlPart) -> Option<ControlAction> {
        let child_response = self
            .children
            .borrow_mut()
            .deliver_event(InputEvent::Mouse(*event));
        if child_response.handled {
            return child_response.action;
        }

        match event {
            MouseEvent::Click {
                button: MouseButton::Left,
                ..
            }
            | MouseEvent::Down {
                button: MouseButton::Left,
                ..
            } => Some(ControlAction::Activated { part: hit_part }),
            _ => None,
        }
    }

    fn handle_keyboard(&self, event: &KeyboardEvent) -> Option<ControlAction> {
        let child_response = self
            .children
            .borrow_mut()
            .deliver_event(InputEvent::Keyboard(*event));
        if child_response.handled {
            return child_response.action;
        }

        if event.is_down()
            && let Some(Key::Enter | Key::Space) = event.key()
        {
            return Some(ControlAction::Activated {
                part: ControlPart::Body,
            });
        }
        None
    }
}

impl<S> ReadValue<String> for Panel<'_, S> {
    fn read_value(&self) -> String {
        self.title().unwrap_or_default()
    }
}
