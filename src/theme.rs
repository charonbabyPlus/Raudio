use std::cell::RefCell;

use gtk::gdk;

/// A full colour scheme. The palettes are drawn from a few well-known apps but
/// stay deliberately unnamed in the UI — they're just swatches to pick from.
pub struct Theme {
    app_bg: &'static str,
    rail_bg: &'static str,
    content_bg: &'static str,
    border: &'static str,
    ink: &'static str,
    accent: &'static str,
    accent_fg: &'static str,
    accent_ink: &'static str,
    dark: bool,
}

pub const THEMES: &[Theme] = &[
    // Ours — ivory + sage green.
    Theme { app_bg: "#F0EDE4", rail_bg: "#ECE8DD", content_bg: "#F7F5EF", border: "#DED9CB",
            ink: "#24221D", accent: "#5F8C6B", accent_fg: "#FFFFFF", accent_ink: "#3C5E47", dark: false },
    // Ivory + clay.
    Theme { app_bg: "#F0EDE6", rail_bg: "#EDE8DE", content_bg: "#FAF8F3", border: "#E0D9CC",
            ink: "#2B2A26", accent: "#CC785C", accent_fg: "#FFFFFF", accent_ink: "#A24E32", dark: false },
    // Clean white + teal.
    Theme { app_bg: "#FFFFFF", rail_bg: "#F7F7F8", content_bg: "#FFFFFF", border: "#E5E5E5",
            ink: "#1F2023", accent: "#10A37F", accent_fg: "#FFFFFF", accent_ink: "#0E8C6D", dark: false },
    // Near-black + bright green.
    Theme { app_bg: "#000000", rail_bg: "#121212", content_bg: "#121212", border: "#282828",
            ink: "#FFFFFF", accent: "#1DB954", accent_fg: "#000000", accent_ink: "#1ED760", dark: true },
    // Dark + warm yellow.
    Theme { app_bg: "#0D0D0D", rail_bg: "#161616", content_bg: "#141414", border: "#2A2A2A",
            ink: "#FFFFFF", accent: "#FFD54A", accent_fg: "#000000", accent_ink: "#FFDB4D", dark: true },
    // Light grey + orange.
    Theme { app_bg: "#F2F2F2", rail_bg: "#FFFFFF", content_bg: "#FFFFFF", border: "#E4E4E4",
            ink: "#333333", accent: "#FF5500", accent_fg: "#FFFFFF", accent_ink: "#E64D00", dark: false },
    // Dark + red.
    Theme { app_bg: "#0F0F0F", rail_bg: "#0F0F0F", content_bg: "#181818", border: "#303030",
            ink: "#FFFFFF", accent: "#FF3B30", accent_fg: "#FFFFFF", accent_ink: "#FF5C54", dark: true },
];

pub const COUNT: usize = THEMES.len();

/// The structural stylesheet, loaded from the embedded GResource.
fn structural() -> String {
    gtk::gio::resources_lookup_data(
        "/dev/raudio/style.css",
        gtk::gio::ResourceLookupFlags::NONE,
    )
    .ok()
    .and_then(|bytes| std::str::from_utf8(&bytes).map(str::to_owned).ok())
    .unwrap_or_default()
}

thread_local! {
    static PROVIDER: RefCell<Option<gtk::CssProvider>> = const { RefCell::new(None) };
}

/// Create the shared stylesheet provider and apply the default theme.
pub fn install(display: &gdk::Display) {
    let provider = gtk::CssProvider::new();
    gtk::style_context_add_provider_for_display(
        display,
        &provider,
        gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );
    PROVIDER.with(|p| *p.borrow_mut() = Some(provider));
    set(0);
}

/// Switch to theme `index`, rebuilding the palette + structural CSS.
pub fn set(index: usize) {
    let t = &THEMES[index % THEMES.len()];
    let css = format!("{}\n{}\n{}", palette_css(t), structural(), swatch_css());
    PROVIDER.with(|p| {
        if let Some(provider) = p.borrow().as_ref() {
            provider.load_from_string(&css);
        }
    });
    adw::StyleManager::default().set_color_scheme(if t.dark {
        adw::ColorScheme::ForceDark
    } else {
        adw::ColorScheme::ForceLight
    });
}

/// The named-colour block the structural stylesheet is written against.
fn palette_css(t: &Theme) -> String {
    format!(
        "@define-color accent_bg_color {accent};
         @define-color accent_fg_color {accent_fg};
         @define-color accent_color {accent_ink};
         @define-color window_bg_color {app_bg};
         @define-color window_fg_color {ink};
         @define-color view_bg_color {content_bg};
         @define-color view_fg_color {ink};
         @define-color app_bg {app_bg};
         @define-color rail_bg {rail_bg};
         @define-color content_bg {content_bg};
         @define-color card_border {border};",
        accent = t.accent,
        accent_fg = t.accent_fg,
        accent_ink = t.accent_ink,
        app_bg = t.app_bg,
        ink = t.ink,
        content_bg = t.content_bg,
        rail_bg = t.rail_bg,
        border = t.border,
    )
}

/// Per-theme swatch backgrounds for the picker (constant across the active
/// theme, so they can't use named colours).
fn swatch_css() -> String {
    let mut css = String::new();
    for (i, t) in THEMES.iter().enumerate() {
        css.push_str(&format!(
            ".swatch-{i} {{ background-image: linear-gradient(135deg, {bg} 0%, {bg} 50%, {acc} 50%, {acc} 100%); }}\n",
            i = i,
            bg = t.content_bg,
            acc = t.accent,
        ));
    }
    css
}
