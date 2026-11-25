//! The `cosmic::Application` panel applet: shows the pending update count in
//! the panel, and offers update / settings controls in a popup.

use std::borrow::Cow;
use std::time::Duration;

use cosmic::app::{Core, Task};
use cosmic::cosmic_config::{Config, CosmicConfigEntry};
use cosmic::iced::futures::channel::oneshot;
use cosmic::iced::platform_specific::shell::wayland::commands::popup::destroy_popup;
use cosmic::iced::{window, Alignment, Subscription};
use cosmic::widget::{button, column, divider, dropdown, icon, row, text};
use cosmic::{Application, Element};

use crate::config::{AppletConfig, INTERVALS_MINUTES};
use crate::updates::{self, CheckResult, Source};

const INTERVAL_LABELS: [&str; 5] = ["5 minutes", "15 minutes", "30 minutes", "1 hour", "2 hours"];
const ICON_NAME: &str = "software-update-available-symbolic";

pub struct ArchUpdatesApplet {
    core: Core,
    popup: Option<window::Id>,
    config_handler: Option<Config>,
    config: AppletConfig,
    result: CheckResult,
    checked_once: bool,
    checking: bool,
    updating: bool,
}

#[derive(Debug, Clone)]
pub enum Message {
    TogglePopup,
    PopupClosed(window::Id),
    ConfigChanged(AppletConfig),
    Tick,
    CheckNow,
    Checked(CheckResult),
    UpdateNow,
    UpdateFinished(Option<String>),
    SourceSelected(usize),
    IntervalSelected(usize),
}

/// Runs blocking `f` on its own thread and awaits the result.
async fn run_blocking<T: Send + 'static>(f: impl FnOnce() -> T + Send + 'static) -> Option<T> {
    let (tx, rx) = oneshot::channel();
    std::thread::spawn(move || {
        let _ = tx.send(f());
    });
    rx.await.ok()
}

impl ArchUpdatesApplet {
    fn start_check(&mut self) -> Task<Message> {
        if self.checking {
            return Task::none();
        }
        self.checking = true;
        let source = self.config.source;
        Task::perform(
            async move {
                run_blocking(move || updates::check(source))
                    .await
                    .unwrap_or_default()
            },
            |r| cosmic::Action::App(Message::Checked(r)),
        )
    }

    fn persist<T>(&mut self, apply: impl FnOnce(&mut AppletConfig, &Config) -> Result<T, cosmic::cosmic_config::Error>, fallback: impl FnOnce(&mut AppletConfig)) {
        match &self.config_handler {
            Some(handler) => {
                if let Err(err) = apply(&mut self.config, handler) {
                    tracing::error!(?err, "failed to persist arch updates config");
                }
            }
            None => fallback(&mut self.config),
        }
    }

    fn headline(&self) -> String {
        if self.checking && !self.checked_once {
            return "Checking for updates…".to_string();
        }
        match self.result.total() {
            1 => "1 update available".to_string(),
            n => format!("{n} updates available"),
        }
    }
}

impl Application for ArchUpdatesApplet {
    type Executor = cosmic::SingleThreadExecutor;
    type Flags = ();
    type Message = Message;
    const APP_ID: &'static str = crate::APP_ID;

    fn core(&self) -> &Core {
        &self.core
    }

    fn core_mut(&mut self) -> &mut Core {
        &mut self.core
    }

    fn init(core: Core, _flags: Self::Flags) -> (Self, Task<Message>) {
        let config_handler = Config::new(crate::APP_ID, AppletConfig::VERSION).ok();
        let config = config_handler
            .as_ref()
            .map(|handler| {
                AppletConfig::get_entry(handler).unwrap_or_else(|(errors, config)| {
                    for err in errors {
                        tracing::error!(?err, "error loading arch updates config");
                    }
                    config
                })
            })
            .unwrap_or_default();

        let mut app = Self {
            core,
            popup: None,
            config_handler,
            config,
            result: CheckResult::default(),
            checked_once: false,
            checking: false,
            updating: false,
        };
        let task = app.start_check();
        (app, task)
    }

    fn style(&self) -> Option<cosmic::iced::theme::Style> {
        Some(cosmic::applet::style())
    }

    fn subscription(&self) -> Subscription<Message> {
        let minutes = u64::from(self.config.interval_minutes.max(1));
        Subscription::batch([
            self.core.watch_config(crate::APP_ID).map(|update| {
                for err in update.errors {
                    tracing::error!(?err, "error watching arch updates config");
                }
                Message::ConfigChanged(update.config)
            }),
            cosmic::iced::time::every(Duration::from_secs(minutes * 60)).map(|_| Message::Tick),
        ])
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::TogglePopup => {
                if let Some(id) = self.popup.take() {
                    return destroy_popup(id);
                }
                return cosmic::surface::surface_task(cosmic::surface::action::app_popup(
                    |_| Default::default(),
                    |app: &mut Self| {
                        let new_id = window::Id::unique();
                        app.popup = Some(new_id);
                        app.core.applet.get_popup_settings(
                            app.core.main_window_id().unwrap(),
                            new_id,
                            None,
                            None,
                            None,
                        )
                    },
                    None,
                ));
            }
            Message::PopupClosed(id) => {
                if self.popup == Some(id) {
                    self.popup = None;
                }
            }
            Message::ConfigChanged(config) => {
                let source_changed = config.source != self.config.source;
                self.config = config;
                if source_changed {
                    return self.start_check();
                }
            }
            Message::Tick | Message::CheckNow => return self.start_check(),
            Message::Checked(result) => {
                self.result = result;
                self.checking = false;
                self.checked_once = true;
            }
            Message::UpdateNow => {
                let Some(command) =
                    updates::update_command(self.config.source, updates::detect_aur_helper())
                else {
                    return Task::none();
                };
                if self.updating {
                    return Task::none();
                }
                self.updating = true;
                let close = self.popup.take().map(destroy_popup);
                let run = Task::perform(
                    async move {
                        run_blocking(move || updates::run_in_terminal(&command).err())
                            .await
                            .flatten()
                    },
                    |err| cosmic::Action::App(Message::UpdateFinished(err)),
                );
                return match close {
                    Some(close) => Task::batch([close, run]),
                    None => run,
                };
            }
            Message::UpdateFinished(err) => {
                self.updating = false;
                if let Some(err) = err {
                    tracing::error!(%err, "update terminal failed");
                    self.result.errors = vec![err];
                    return Task::none();
                }
                return self.start_check();
            }
            Message::SourceSelected(index) => {
                if let Some(source) = Source::ALL.get(index).copied() {
                    self.persist(
                        |c, h| c.set_source(h, source),
                        |c| c.source = source,
                    );
                    return self.start_check();
                }
            }
            Message::IntervalSelected(index) => {
                if let Some(minutes) = INTERVALS_MINUTES.get(index).copied() {
                    self.persist(
                        |c, h| c.set_interval_minutes(h, minutes),
                        |c| c.interval_minutes = minutes,
                    );
                }
            }
        }
        Task::none()
    }

    fn view(&self) -> Element<'_, Message> {
        let count = self.result.total().to_string();
        let (size, _) = self.core.applet.suggested_size(true);
        let glyph = icon::from_name(ICON_NAME).size(size).symbolic(true);

        let content: Element<'_, Message> = if self.core.applet.is_horizontal() {
            row![glyph, self.core.applet.text(count)]
                .spacing(4)
                .align_y(Alignment::Center)
                .into()
        } else {
            column![glyph, self.core.applet.text(count)]
                .spacing(2)
                .align_x(Alignment::Center)
                .into()
        };

        let button = button::custom(content)
            .on_press_down(Message::TogglePopup)
            .class(cosmic::theme::Button::AppletIcon);

        self.core.applet.autosize_window(button).into()
    }

    fn view_window(&self, _id: window::Id) -> Element<'_, Message> {
        let source = self.config.source;
        let helper = updates::detect_aur_helper();
        let can_update = self.result.total() > 0
            && !self.updating
            && updates::update_command(source, helper).is_some();

        let mut content = column![text::heading(self.headline())].spacing(8).padding(12);

        if source == Source::Both {
            content = content
                .push(text(format!("Pacman: {}", self.result.pacman)))
                .push(text(format!("AUR: {}", self.result.aur)));
        }
        for err in &self.result.errors {
            content = content.push(text::caption(err.clone()));
        }

        let update_label = if self.updating { "Updating…" } else { "Update now" };
        let check_label = if self.checking { "Checking…" } else { "Check now" };
        content = content
            .push(
                row![
                    button::suggested(update_label).on_press_maybe(can_update.then_some(Message::UpdateNow)),
                    button::standard(check_label)
                        .on_press_maybe((!self.checking).then_some(Message::CheckNow)),
                ]
                .spacing(8),
            )
            .push(divider::horizontal::default())
            .push(text::heading("Settings"))
            .push(
                row![
                    text("Check"),
                    dropdown(
                        Cow::Borrowed(Source::LABELS.as_slice()),
                        Some(source.index()),
                        Message::SourceSelected,
                    ),
                ]
                .spacing(8)
                .align_y(Alignment::Center),
            )
            .push(
                row![
                    text("Every"),
                    dropdown(
                        Cow::Borrowed(INTERVAL_LABELS.as_slice()),
                        INTERVALS_MINUTES
                            .iter()
                            .position(|m| *m == self.config.interval_minutes),
                        Message::IntervalSelected,
                    ),
                ]
                .spacing(8)
                .align_y(Alignment::Center),
            );

        self.core.applet.popup_container(content).into()
    }

    fn on_close_requested(&self, id: window::Id) -> Option<Message> {
        Some(Message::PopupClosed(id))
    }
}
