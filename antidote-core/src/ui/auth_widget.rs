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
use agg_gui::widgets::button::{Button, ButtonTheme};
use agg_gui::widgets::label::{Label, LabelAlign};
use agg_gui::widgets::text_field::TextField;
use agg_gui::{DrawCtx, Event, EventResult, Rect, Widget};

use crate::db::auth::OAuthProvider;
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

        let google_btn = oauth_button(OAuthProvider::Google, font.clone(), model.clone());
        // Facebook and Apple are intentionally hidden until those providers
        // are actually configured in Supabase (see `db/README.md`). The
        // OAuthProvider enum still has the variants so re-enabling them is
        // a one-line change once their Client IDs land.

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
            google_btn,
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

/// Build a "Sign in with X" button that, on click, asks the platform shell
/// to open Supabase's `/auth/v1/authorize` URL for the chosen provider.
/// Each provider must be configured (Client ID + Secret) in the Supabase
/// Dashboard before the round trip will succeed; until then the user lands
/// on a Supabase error page. See `db/README.md`.
///
/// Google's button uses the white-on-dark-text styling from Google's brand
/// guidelines (one of two officially approved styles). Other providers fall
/// back to the standard secondary-button look until we ship dedicated
/// branding for each.
fn oauth_button(provider: OAuthProvider, font: Arc<Font>, model: SharedModel) -> Box<dyn Widget> {
    let label = format!("Sign in with {}", provider.display_name());
    let click = move || {
        let mut m = model.borrow_mut();
        // The platform shell looks up its own redirect target; here we just
        // mark which provider was requested. The shell builds the URL via
        // `AuthClient::oauth_url` (it knows its own origin) and assigns to
        // `pending_open_url`. Doing it client-side keeps antidote-core
        // free of any platform/redirect-URL knowledge.
        m.auth.pending_oauth = Some(provider);
        m.auth.last_error = None;
    };
    match provider {
        OAuthProvider::Google => Box::new(
            Button::new(&label, font)
                .with_font_size(16.0)
                .with_theme(google_button_theme())
                .with_min_size(Size::new(COL_W, 40.0))
                .on_click(click),
        ),
        _ => secondary_button(&label, font, click),
    }
}

/// Google's brand-approved white button: white surface + Google's neutral
/// dark-grey text (`#3C4043`). Hover/pressed states from their guidelines
/// are subtle grey shifts. Border radius matches the rest of our menu
/// buttons.
fn google_button_theme() -> ButtonTheme {
    ButtonTheme {
        background: Color::rgb(1.0, 1.0, 1.0),
        background_hovered: Color::rgb(0.95, 0.96, 0.97),
        background_pressed: Color::rgb(0.88, 0.89, 0.91),
        label_color: Color::rgb(0.235, 0.251, 0.263),
        border_radius: 6.0,
        focus_ring_color: Color::rgba(0.259, 0.522, 0.957, 0.55),
        focus_ring_width: 2.5,
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
