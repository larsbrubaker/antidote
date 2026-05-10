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

#[cfg(target_arch = "wasm32")]
use crate::db::auth::OAuthProvider;
use crate::game::state::Phase;
use crate::ui::game_model::{MenuView, SharedModel};
#[cfg(target_arch = "wasm32")]
use crate::ui::google_signin_button::GoogleSignInButton;
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

        // Status label: shows `auth.notice` in green when present, otherwise
        // `auth.last_error` in red. `refresh_dynamic_text` swaps both text
        // and color on each layout pass.
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

        let recover_model = model.clone();
        let recover_email = email_buf.clone();
        let forgot_btn = secondary_button("Forgot password?", font.clone(), move || {
            request_password_reset(&recover_model, &recover_email);
        });

        let back_model = model.clone();
        let back_btn = secondary_button("Back", font.clone(), move || {
            let mut m = back_model.borrow_mut();
            m.auth.last_error = None;
            m.auth.notice = None;
            m.menu_view = MenuView::Main;
        });

        // OAuth round trips need a same-origin redirect target to capture
        // the hash fragment with the access tokens. The browser shell can
        // do this trivially (its origin is the deployed page); the native
        // shell would need a localhost-loopback HTTP listener, which we
        // haven't built yet. Until then, skip the OAuth buttons on
        // native — the user lands on a Supabase callback page they
        // can't return from. Email/password works on both targets.
        //
        // Facebook and Apple variants stay hidden in either build until
        // their providers are configured in Supabase (see db/README.md).
        let mut children: Vec<Box<dyn Widget>> = vec![
            header_label("Sign in", font.clone(), 30.0),
            body_label(
                "Sign in to keep your scores across devices.",
                font.clone(),
                Some(Color::rgba(0.75, 0.82, 0.95, 1.0)),
            ),
            Box::new(email_field),
            Box::new(password_field),
            Box::new(error_label),
            signin_btn,
            signup_btn,
            forgot_btn,
        ];
        #[cfg(target_arch = "wasm32")]
        children.push(oauth_button(
            OAuthProvider::Google,
            font.clone(),
            model.clone(),
        ));
        children.push(back_btn);
        let _ = &font; // silence "unused after move" on cfg(not(wasm32))

        Self {
            bounds: Rect::default(),
            children,
            model,
        }
    }

    fn refresh_dynamic_text(&mut self) {
        // Index 4 = status label. `auth.notice` (green) wins over
        // `auth.last_error` (red) so success confirmations don't get
        // hidden by a stale error from a previous attempt.
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
        self.children[4].set_label_text(&text);
        self.children[4].set_label_color(color);
    }
}

/// Build a "Sign in with X" button that, on click, asks the platform shell
/// to open Supabase's `/auth/v1/authorize` URL for the chosen provider.
/// Each provider must be configured (Client ID + Secret) in the Supabase
/// Dashboard before the round trip will succeed; until then the user lands
/// on a Supabase error page. See `db/README.md`.
///
/// Google's button is rendered through the dedicated [`GoogleSignInButton`]
/// widget so we can paint the official multicolor "G" SVG next to the
/// label — Google's brand guidelines mandate the logo. Other providers
/// fall back to the standard secondary-button look until we ship matching
/// branding for each.
///
/// Gated to wasm32 because the OAuth round trip needs a same-origin
/// browser redirect to capture the access-token fragment; the native
/// shell can't do that until a localhost-loopback handler lands.
#[cfg(target_arch = "wasm32")]
fn oauth_button(provider: OAuthProvider, font: Arc<Font>, model: SharedModel) -> Box<dyn Widget> {
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
        OAuthProvider::Google => Box::new(GoogleSignInButton::new(font, click)),
        _ => {
            let label = format!("Sign in with {}", provider.display_name());
            secondary_button(&label, font, click)
        }
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
        m.auth.notice = None;
        return;
    }
    m.auth.pending = true;
    m.auth.last_error = None;
    m.auth.notice = None;
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

/// Stash the typed email on `auth.pending_recover_email` so the platform
/// shell's per-frame [`crate::ui::drain_pending_password_reset`] hook can
/// fire the actual REST call with the right `redirect_to`. Validates that
/// the email field isn't empty up-front so the user gets immediate feedback.
fn request_password_reset(model: &SharedModel, email_buf: &Rc<RefCell<String>>) {
    let mut m = model.borrow_mut();
    if m.auth.recover_pending {
        return;
    }
    let email = email_buf.borrow().clone();
    if email.trim().is_empty() {
        m.auth.last_error = Some("Enter your email above, then tap Forgot password.".to_owned());
        m.auth.notice = None;
        return;
    }
    m.auth.recover_pending = true;
    m.auth.last_error = None;
    m.auth.notice = None;
    m.auth.pending_recover_email = Some(email);
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
