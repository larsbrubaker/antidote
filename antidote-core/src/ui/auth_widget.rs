//! `SignInOverlay` — email/password sign-in & sign-up form.
//!
//! Shown as a sub-view of the main menu when
//! [`MenuView`](crate::ui::game_model::MenuView) is `SignIn`. Email + password
//! `TextField`s feed `Rc<RefCell<String>>` cells via their `on_change`
//! callbacks; the Sign-in / Sign-up buttons read those cells and dispatch
//! through [`AuthClient`](crate::db::auth::AuthClient).
//!
//! Async results land on `services.inbox`. The drain hook in
//! [`crate::ui::drain_db_inbox`] writes them into [`AuthState`] and resets
//! `pending = false`. The error label below the form re-renders from
//! `auth.last_error` whenever the layout pass runs.

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

pub struct SignInOverlay {
    bounds: Rect,
    children: Vec<Box<dyn Widget>>,
    model: SharedModel,
}

impl SignInOverlay {
    pub fn new(model: SharedModel, font: Arc<Font>) -> Self {
        // Shared text buffers — TextField on_change callbacks write into
        // them; the submit buttons read them.
        let email_buf: Rc<RefCell<String>> = Rc::new(RefCell::new(String::new()));
        let password_buf: Rc<RefCell<String>> = Rc::new(RefCell::new(String::new()));

        let email_clone = email_buf.clone();
        let email_field = TextField::new(font.clone())
            .with_font_size(16.0)
            .with_padding(8.0)
            .with_placeholder("Email")
            .with_min_size(Size::new(COL_W, 38.0))
            .with_margin(Insets::all(0.0))
            .on_change(move |s| *email_clone.borrow_mut() = s.to_owned());

        let password_clone = password_buf.clone();
        let password_field = TextField::new(font.clone())
            .with_font_size(16.0)
            .with_padding(8.0)
            .with_placeholder("Password")
            .with_password_mode(true)
            .with_min_size(Size::new(COL_W, 38.0))
            .with_margin(Insets::all(0.0))
            .on_change(move |s| *password_clone.borrow_mut() = s.to_owned());

        let error_label = Label::new("", font.clone())
            .with_font_size(13.0)
            .with_align(LabelAlign::Center)
            .with_color(Color::rgba(1.0, 0.45, 0.45, 1.0))
            .with_has_backbuffer(false)
            .with_wrap(true)
            .with_min_size(Size::new(COL_W, 18.0));

        let signin_model = model.clone();
        let signin_email = email_buf.clone();
        let signin_password = password_buf.clone();
        let signin_btn = primary_button("Sign in", font.clone(), move || {
            submit(&signin_model, &signin_email, &signin_password, false);
        });

        let signup_model = model.clone();
        let signup_email = email_buf.clone();
        let signup_password = password_buf.clone();
        let signup_btn = secondary_button("Create account", font.clone(), move || {
            submit(&signup_model, &signup_email, &signup_password, true);
        });

        let back_model = model.clone();
        let back_btn = secondary_button("Back", font.clone(), move || {
            let mut m = back_model.borrow_mut();
            m.auth.last_error = None;
            m.menu_view = MenuView::Main;
        });

        let children: Vec<Box<dyn Widget>> = vec![
            header_label("Sign in", font.clone(), 30.0),
            body_label(
                "Sign in to keep your scores across devices.",
                font,
                Some(Color::rgba(0.75, 0.82, 0.95, 1.0)),
            ),
            Box::new(email_field),
            Box::new(password_field),
            Box::new(error_label),
            signin_btn,
            signup_btn,
            back_btn,
        ];

        Self {
            bounds: Rect::default(),
            children,
            model,
        }
    }

    fn refresh_dynamic_text(&mut self) {
        // Index 4 = error label. Mirror auth.last_error.
        let text = self
            .model
            .borrow()
            .auth
            .last_error
            .clone()
            .unwrap_or_default();
        self.children[4].set_label_text(&text);
    }
}

fn submit(
    model: &SharedModel,
    email_buf: &Rc<RefCell<String>>,
    password_buf: &Rc<RefCell<String>>,
    sign_up: bool,
) {
    let mut m = model.borrow_mut();
    if m.auth.pending {
        return;
    }
    let email = email_buf.borrow().clone();
    let password = password_buf.borrow().clone();
    if email.trim().is_empty() || password.is_empty() {
        m.auth.last_error = Some("email and password required".to_owned());
        return;
    }
    m.auth.pending = true;
    m.auth.last_error = None;
    if sign_up {
        m.services
            .auth
            .sign_up_async(&email, &password, &m.services.inbox);
    } else {
        m.services
            .auth
            .sign_in_async(&email, &password, &m.services.inbox);
    }
}

impl Widget for SignInOverlay {
    fn type_name(&self) -> &'static str {
        "SignInOverlay"
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
        m.world.phase == Phase::Start && m.menu_view == MenuView::SignIn
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
        // Backdrop swallows mouse events so the play area underneath
        // doesn't react. Keys still bubble to the global handler so Esc/P
        // works.
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
