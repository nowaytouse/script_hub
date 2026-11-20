//! An unlocked workspace may be temporary or backed by the encrypted store.

use super::super::{Palette, localized, panel};
use super::view::{description, field, row, section};
use super::{QuitAfterLibraryReview, Store, Subscriptions, Workspace, clear, message};
use crate::desktop::i18n::{Text, text};
use gpui::{Context, Div, ParentElement, Styled, px};
use std::sync::Arc;

#[derive(Clone, Default)]
pub(super) enum Library {
    #[default]
    Locked,
    Temporary,
    Encrypted(Arc<Store>),
}

impl Library {
    pub(super) fn is_open(&self) -> bool {
        !matches!(self, Self::Locked)
    }

    pub(super) fn writer(&self) -> Option<Self> {
        self.is_open().then(|| self.clone())
    }

    pub(super) fn save(&self, workspace: &Workspace) -> Result<(), String> {
        match self {
            Self::Temporary => workspace.encode().map(drop).map_err(message),
            Self::Encrypted(store) => store.save(workspace).map_err(message),
            Self::Locked => Err("Unlock the library first.".into()),
        }
    }
}

impl Subscriptions {
    pub(in crate::gpui_ui) fn has_unsaved_session(&self) -> bool {
        matches!(self.library, Library::Temporary)
            && !(self.workspace.subscriptions.is_empty() && self.workspace.collections.is_empty())
    }

    pub(in crate::gpui_ui) fn confirm_end_session(&mut self, cx: &mut Context<Self>) {
        self.request_session_end(true, cx);
    }

    fn request_session_end(&mut self, quit: bool, cx: &mut Context<Self>) {
        if self.has_unsaved_session() {
            self.session_quit = quit;
            self.confirm_session_discard = true;
            cx.notify();
        }
    }

    pub(super) fn finish_session_quit(&mut self, cx: &mut Context<Self>) {
        if std::mem::take(&mut self.session_quit) {
            cx.emit(QuitAfterLibraryReview);
        }
    }

    pub(super) fn start_temporary_library(&mut self, cx: &mut Context<Self>) {
        if !self.library.is_open() {
            self.library = Library::Temporary;
            self.clear_library_passwords(cx);
            self.notice = None;
            cx.notify();
        }
    }

    pub(super) fn clear_library_passwords(&self, cx: &mut Context<Self>) {
        clear(&self.password, cx);
        clear(&self.confirmation, cx);
    }

    pub(super) fn request_library_lock(&mut self, cx: &mut Context<Self>) {
        if self.has_unsaved_session() {
            self.request_session_end(false, cx);
        } else {
            self.lock(cx);
        }
    }

    pub(super) fn session_banner(&self, palette: Palette, cx: &Context<Self>) -> Option<Div> {
        if !matches!(self.library, Library::Temporary) {
            return None;
        }
        let translate = |key| text(self.language, key);
        let local = |en, zh, ja| localized(self.language, en, zh, ja);
        let title = local("Temporary library", "临时订阅库", "一時ライブラリ");
        let save = translate(if self.session_quit && self.session_password_form {
            Text::SettingsSaveAndQuit
        } else {
            Text::WorkspaceCreateStorage
        });
        let discard = translate(if self.session_quit {
            Text::SettingsDiscardAndQuit
        } else {
            Text::WorkspaceDiscard
        });
        let cancel = translate(Text::Cancel);
        let card = panel(palette).w_full().p_4().gap(px(9.));
        let card = if self.confirm_session_discard {
            card.child(section(translate(Text::WorkspaceUnsaved), palette))
                .child(description(local(
                    "Discard the temporary subscriptions, collections and changes from this session?",
                    "是否丢弃本次会话的临时订阅、合集和更改？",
                    "このセッションの一時的なサブスクリプション、コレクションと変更を破棄しますか？",
                ), palette))
                .child(row()
                    .child(self.action(
                        "discard-temporary-library", discard, palette, false, cx, Self::lock,
                    ))
                    .child(self.action("cancel-discard-library", cancel, palette, true, cx, |view, cx| {
                        view.confirm_session_discard = false;
                        view.session_quit = false;
                        cx.notify();
                    })))
        } else {
            card.child(section(title, palette)).child(description(
                local(
                    "The library is held in memory for this session.",
                    "订阅库仅存于本次会话的内存中。",
                    "ライブラリはこのセッションのメモリーに保持されます。",
                ),
                palette,
            ))
        };
        if self.storage_exists {
            return Some(card.child(description(local(
                "An encrypted library already exists. Lock the temporary library before opening it.",
                "设备已有加密订阅库。请先关闭临时库，再解锁加密库。",
                "暗号化ライブラリが既にあります。開く前に一時ライブラリを閉じてください。",
            ), palette)));
        }

        if self.session_password_form {
            Some(
                card.child(field(
                    translate(Text::WorkspacePassword),
                    &self.password,
                    palette,
                ))
                .child(field(
                    translate(Text::WorkspacePasswordConfirm),
                    &self.confirmation,
                    palette,
                ))
                .child(
                    row()
                        .child(self.action(
                            "save-temporary-encrypted",
                            save,
                            palette,
                            true,
                            cx,
                            Self::open_store,
                        ))
                        .child(self.action(
                            "cancel-temporary-encryption",
                            cancel,
                            palette,
                            false,
                            cx,
                            |view, cx| {
                                view.session_password_form = false;
                                view.session_quit = false;
                                view.clear_library_passwords(cx);
                                cx.notify();
                            },
                        )),
                ),
            )
        } else {
            Some(card.child(row().child(self.action(
                "save-temporary-library",
                save,
                palette,
                true,
                cx,
                |view, cx| {
                    view.session_password_form = true;
                    view.confirm_session_discard = false;
                    cx.notify();
                },
            ))))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::relay::{
        library::{Collection, Entry},
        model::Subscription,
    };

    #[test]
    fn temporary_library_validates_saves_without_a_store() {
        let mut data = Workspace::default();
        assert!(Library::Locked.writer().is_none());
        let writer = Library::Temporary.writer().unwrap();
        let mut members = Vec::new();
        for name in ["first", "second"] {
            members.push(
                data.add_entry(Entry {
                    id: 0,
                    name: name.to_owned(),
                    source: String::new(),
                    source_file: String::new(),
                    content: format!("{name} = socks5, {name}.invalid, 1080\n"),
                    processing: Default::default(),
                    publish: Default::default(),
                })
                .unwrap(),
            );
        }
        let id = data
            .add_collection(Collection {
                id: 0,
                name: "Temporary collection".to_owned(),
                members,
                processing: Default::default(),
                publish: Default::default(),
            })
            .unwrap();
        assert!(writer.save(&data).is_ok());
        let merged = data.merge(id).unwrap();
        assert_eq!(Subscription::parse(&merged).unwrap().nodes().len(), 2);
        data.version = 0;
        assert!(writer.save(&data).is_err());
    }
}
