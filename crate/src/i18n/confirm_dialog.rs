//! `crate::confirm_dialog` localisation strings.
//!
//! These are the dialog's **own** user-visible defaults — the title,
//! body message and the two button labels. A host that supplies its
//! own text via [`DialogConfig::new`](crate::confirm_dialog::DialogConfig::new),
//! [`with_confirm_label`](crate::confirm_dialog::DialogConfig::with_confirm_label)
//! or [`with_cancel_label`](crate::confirm_dialog::DialogConfig::with_cancel_label)
//! always wins; these catalogue strings only fill in the blanks the
//! host left empty. See `confirm_dialog::config` for the precedence rule.

#![allow(missing_docs)]

use super::Locale;

/// Default text for a confirm dialog when the host has not supplied an
/// explicit override.
#[derive(Debug)]
pub struct Strings {
    pub title: &'static str,   // "Confirm"
    pub message: &'static str, // "Are you sure?"
    pub confirm: &'static str, // "Confirm"
    pub cancel: &'static str,  // "Cancel"
}

pub const EN: Strings = Strings {
    title: "Confirm",
    message: "Are you sure?",
    confirm: "Confirm",
    cancel: "Cancel",
};

pub const RU: Strings = Strings {
    title: "Подтверждение",
    message: "Вы уверены?",
    confirm: "Подтвердить",
    cancel: "Отмена",
};

pub fn strings(locale: Locale) -> &'static Strings {
    match locale {
        Locale::En => &EN,
        Locale::Ru => &RU,
    }
}
