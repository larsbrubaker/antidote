//! `SetPasswordOverlay` — the "set a new password" form shown after a user
//! follows a recovery email link.
//!
//! Reached only via [`MenuView::SetPassword`], which the wasm shell sets
//! when it spots `&type=recovery` in the callback URL hash. The recovery
//! access token is stashed on `auth.recovery_access_token` and is what
//! bearer-authenticates the `PUT /auth/v1/user` call. On success we install
//! the recovery session as a normal `Session`, so the user lands on the
//! main menu signed in with the new password.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use agg_gui::color::Color;
use agg_gui::geometry::Size;
use agg_gui::layout_props::Insets;
use agg_gui::text::Font;
use agg_gui::widgets::label::{Label, LabelAlign};
use agg_gui::widgets::text_field::TextField;
use agg_gui::{DrawCtx, Event, EventResult, Rect, Widget};

use crate::game::state::Phase;
use crate::ui::game_model::{MenuView, SharedModel};
use crate::ui::menu_widget::{
    body_label, header_label, layout_centered_column, paint_backdrop, primary_button,
    secondary_button, COL_W,
};

pub struct SetPasswordOverlay {
    bounds: Rect,
    children: Vec<Box<dyn Widget>>,
    model: SharedModel,
}

impl SetPasswordOverlay {
    pub fn new(model: SharedModel, font: Arc<Font>) -> Self {
        let password_buf: Rc<RefCell<String>> = Rc::new(RefCell::new(String::new()));

        let password_clone = password_buf.clone();
        let password_field = TextField::new(font.clone())
            .with_font_size(16.0)
            .with_padding(8.0)
            .with_placeholder("New password")
            .with_password_mode(true)
            .with_min_size(Size::new(COL_W, 38.0))
            .with_margin(Insets::all(0.0))
            .on_change(move |s| *password_clone.borrow_mut() = s.to_owned());

        let status_label = Label::new("", font.clone())
            .with_font_size(13.0)
            .with_align(LabelAlign::Center)
            .with_color(Color::rgba(1.0, 0.45, 0.45, 1.0))
            .with_has_backbuffer(false)
            .with_wrap(true)
            .with_min_size(Size::new(COL_W, 18.0));

        let submit_model = model.clone();
        let submit_password = password_buf.clone();
        let submit_btn = primary_button("Set password", font.clone(), move || {
            submit_new_password(&submit_model, &submit_password);
        });

        let cancel_model = model.clone();
        let cancel_btn = secondary_button("Cancel", font.clone(), move || {
            let mut m = cancel_model.borrow_mut();
            m.auth.recovery_access_token = None;
            m.auth.recovery_refresh_token = None;
            m.auth.recovery_expires_in = None;
            m.auth.last_error = None;
            m.auth.notice = None;
            m.menu_view = MenuView::Main;
        });

        let children: Vec<Box<dyn Widget>> = vec![
            header_label("Set new password", font.clone(), 30.0),
            body_label(
                "You followed a password-reset link. Pick a new password to finish.",
                font.clone(),
                Some(Color::rgba(0.75, 0.82, 0.95, 1.0)),
            ),
            Box::new(password_field),
            Box::new(status_label),
            submit_btn,
            cancel_btn,
        ];

        Self {
            bounds: Rect::default(),
            children,
            model,
        }
    }

    fn refresh_dynamic_text(&mut self) {
        // Index 3 = status label. Same notice/error pattern as SignInOverlay.
        let (text, color) = {
            let m = self.model.borrow();
            if let Some(notice) = m.auth.notice.as_deref() {
                (notice.to_owned(), Color::rgba(0.55, 0.95, 0.65, 1.0))
            } else if let Some(err) = m.auth.last_error.as_deref() {
                (err.to_owned(), Color::rgba(1.0, 0.45, 0.45, 1.0))
            } else {
                (String::new(), Color::rgba(1.0, 0.45, 0.45, 1.0))
            }
        };
        self.children[3].set_label_text(&text);
        self.children[3].set_label_color(color);
    }
}

fn submit_new_password(model: &SharedModel, password_buf: &Rc<RefCell<String>>) {
    let mut m = model.borrow_mut();
    if m.auth.pending {
        return;
    }
    let password = password_buf.borrow().clone();
    if password.len() < 6 {
        m.auth.last_error = Some("Password must be at least 6 characters.".to_owned());
        m.auth.notice = None;
        return;
    }
    let Some(token) = m.auth.recovery_access_token.clone() else {
        m.auth.last_error = Some("Recovery session expired. Request a new reset link.".to_owned());
        return;
    };
    m.auth.pending = true;
    m.auth.last_error = None;
    m.auth.notice = None;
    m.services
        .auth
        .update_password_async(&token, &password, &m.services.inbox);
}

impl Widget for SetPasswordOverlay {
    fn type_name(&self) -> &'static str {
        "SetPasswordOverlay"
    }
    fn bounds(&self) -> Rect {
        self.bounds
    }
    fn set_bounds(&mut self, bounds: Rect) {
        self.bounds = bounds;
    }
    fn children(&self) -> &[Box<dyn Widget>] {
        &self.children
    }
    fn children_mut(&mut self) -> &mut Vec<Box<dyn Widget>> {
        &mut self.children
    }
    fn is_visible(&self) -> bool {
        let m = self.model.borrow();
        m.world.phase == Phase::Start && m.menu_view == MenuView::SetPassword
    }
    fn layout(&mut self, available: Size) -> Size {
        self.refresh_dynamic_text();
        layout_centered_column(&mut self.children, available.width, available.height);
        available
    }
    fn paint(&mut self, ctx: &mut dyn DrawCtx) {
        paint_backdrop(ctx, self.bounds.width, self.bounds.height);
    }
    fn on_event(&mut self, event: &Event) -> EventResult {
        match event {
            Event::MouseMove { .. }
            | Event::MouseDown { .. }
            | Event::MouseUp { .. }
            | Event::MouseWheel { .. } => EventResult::Consumed,
            _ => EventResult::Ignored,
        }
    }
    fn needs_draw(&self) -> bool {
        self.is_visible()
    }
}
