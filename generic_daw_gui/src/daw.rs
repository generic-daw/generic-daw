use crate::{
    arrangement_view::{ArrangementView, Message as ArrangementMessage},
    clap_host_view::{ClapHostView, Message as ClapHostMessage},
    widget::VSplit,
};
use fragile::Fragile;
use generic_daw_core::{
    Denominator, InterleavedAudio, Meter, Numerator, VARIANTS as _,
    clap_host::{self, PluginDescriptor, clack_host::bundle::PluginBundle},
};
use home::home_dir;
use iced::{
    Alignment::Center,
    Element, Event, Length, Subscription, Task, Theme,
    event::{self, Status},
    keyboard,
    widget::{button, column, horizontal_space, pick_list, row, scrollable, svg, toggler},
    window,
};
use iced_aw::number_input;
use iced_file_tree::file_tree;
use rfd::{AsyncFileDialog, FileHandle};
use std::{
    collections::BTreeMap,
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::Ordering::{AcqRel, Acquire, Release},
    },
};

#[derive(Clone, Debug)]
pub enum Message {
    Animate,
    ThemeChanged(Theme),
    ClapHost(ClapHostMessage),
    Arrangement(ArrangementMessage),
    LoadPlugin(PluginDescriptor),
    LoadSamplesButton,
    LoadSample(PathBuf),
    ExportButton,
    TogglePlay,
    Stop,
    New,
    BpmChanged(u16),
    NumeratorChanged(Numerator),
    DenominatorChanged(Denominator),
    ToggleMetronome,
}

pub struct Daw {
    arrangement: ArrangementView,
    clap_host: ClapHostView,
    plugins: BTreeMap<PluginDescriptor, PluginBundle>,
    meter: Arc<Meter>,
    theme: Theme,
}

impl Default for Daw {
    fn default() -> Self {
        let (meter, arrangement) = ArrangementView::create();
        let plugins = clap_host::get_installed_plugins();

        Self {
            arrangement,
            clap_host: ClapHostView::default(),
            plugins,
            meter,
            theme: Theme::Dark,
        }
    }
}

impl Daw {
    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Animate => {}
            Message::ThemeChanged(theme) => self.theme = theme,
            Message::ClapHost(message) => {
                return self.clap_host.update(message).map(Message::ClapHost);
            }
            Message::Arrangement(message) => {
                return self.arrangement.update(message).map(Message::Arrangement);
            }
            Message::LoadPlugin(name) => {
                let (gui, gui_receiver, audio_processor) = clap_host::init(
                    &self.plugins[&name],
                    &name,
                    f64::from(self.meter.sample_rate),
                    self.meter.buffer_size,
                );
                let gui = Fragile::new(gui);

                return Task::batch([
                    Task::done(Message::Arrangement(ArrangementMessage::LoadedPlugin(
                        Arc::new(Mutex::new(audio_processor)),
                    ))),
                    Task::done(Message::ClapHost(ClapHostMessage::Opened(Arc::new(
                        Mutex::new((gui, gui_receiver)),
                    )))),
                ]);
            }
            Message::LoadSamplesButton => {
                return Task::future(AsyncFileDialog::new().pick_files()).and_then(|paths| {
                    Task::batch(
                        paths
                            .iter()
                            .map(FileHandle::path)
                            .map(PathBuf::from)
                            .map(Message::LoadSample)
                            .map(Task::done),
                    )
                });
            }
            Message::LoadSample(path) => {
                let meter = self.meter.clone();
                return Task::future(tokio::task::spawn_blocking(move || {
                    InterleavedAudio::create(path, &meter)
                }))
                .and_then(Task::done)
                .and_then(Task::done)
                .map(ArrangementMessage::LoadedSample)
                .map(Message::Arrangement);
            }
            Message::ExportButton => {
                return Task::future(
                    AsyncFileDialog::new()
                        .add_filter("Wave File", &["wav"])
                        .save_file(),
                )
                .and_then(Task::done)
                .map(ArrangementMessage::Export)
                .map(Message::Arrangement);
            }
            Message::TogglePlay => {
                self.meter.playing.fetch_not(AcqRel);
            }
            Message::Stop => {
                self.meter.playing.store(false, Release);
                self.meter.sample.store(0, Release);
                self.arrangement.stop();
            }
            Message::New => *self = Self::default(),
            Message::BpmChanged(bpm) => self.meter.bpm.store(bpm, Release),
            Message::NumeratorChanged(new_numerator) => {
                self.meter.numerator.store(new_numerator, Release);
            }
            Message::DenominatorChanged(new_denominator) => {
                self.meter.denominator.store(new_denominator, Release);
            }
            Message::ToggleMetronome => {
                self.meter.metronome.fetch_not(AcqRel);
            }
        }

        Task::none()
    }

    pub fn view(&self) -> Element<'_, Message> {
        let stop_handle = svg::Handle::from_path("assets/material-symbols--stop-rounded.svg");
        let play_pause_handle = svg::Handle::from_path(if self.meter.playing.load(Acquire) {
            "assets/material-symbols--pause-rounded.svg"
        } else {
            "assets/material-symbols--play-arrow-rounded.svg"
        });

        let controls = row![
            row![
                button("Load Samples").on_press(Message::LoadSamplesButton),
                button("Export").on_press(Message::ExportButton),
                button("New").on_press(Message::New),
            ],
            row![
                button(
                    svg(play_pause_handle)
                        .style(|theme: &Theme, _| svg::Style {
                            color: Some(theme.extended_palette().secondary.base.text)
                        })
                        .width(Length::Shrink)
                        .height(Length::Fixed(21.0))
                )
                .on_press(Message::TogglePlay),
                button(
                    svg(stop_handle)
                        .style(|theme: &Theme, _| svg::Style {
                            color: Some(theme.extended_palette().secondary.base.text)
                        })
                        .width(Length::Shrink)
                        .height(Length::Fixed(21.0))
                )
                .on_press(Message::Stop),
            ],
            row![
                pick_list(
                    Numerator::VARIANTS,
                    Some(self.meter.numerator.load(Acquire)),
                    Message::NumeratorChanged
                )
                .width(50),
                pick_list(
                    Denominator::VARIANTS,
                    Some(self.meter.denominator.load(Acquire)),
                    Message::DenominatorChanged
                )
                .width(50),
            ],
            number_input(&self.meter.bpm.load(Acquire), 30..=600, Message::BpmChanged).width(50),
            toggler(self.meter.metronome.load(Acquire))
                .label("Metronome")
                .on_toggle(|_| Message::ToggleMetronome),
            horizontal_space(),
            pick_list(Theme::ALL, Some(&self.theme), Message::ThemeChanged),
            pick_list(
                self.plugins.keys().collect::<Box<[_]>>(),
                Option::<&PluginDescriptor>::None,
                |p| Message::LoadPlugin(p.to_owned())
            )
            .placeholder("Load Plugin")
            .style(|t, s| pick_list::Style {
                placeholder_color: pick_list::default(t, s).text_color,
                ..pick_list::default(t, s)
            })
        ]
        .spacing(20)
        .align_y(Center);

        let content = column![
            controls,
            VSplit::new(
                scrollable(file_tree(home_dir().unwrap()).on_double_click(Message::LoadSample),),
                self.arrangement.view().map(Message::Arrangement)
            )
            .split(0.25)
        ]
        .padding(20)
        .spacing(20);

        content.into()
    }

    pub fn subscription(&self) -> Subscription<Message> {
        let animate = if self.meter.playing.load(Acquire) {
            window::frames().map(|_| Message::Animate)
        } else {
            Subscription::none()
        };

        Subscription::batch([
            animate,
            ClapHostView::subscription().map(Message::ClapHost),
            event::listen_with(|e, s, _| match s {
                Status::Ignored => match e {
                    Event::Keyboard(keyboard::Event::KeyPressed { key, modifiers, .. }) => {
                        match (modifiers.command(), modifiers.shift(), modifiers.alt()) {
                            (false, false, false) => match key {
                                keyboard::Key::Named(keyboard::key::Named::Space) => {
                                    Some(Message::TogglePlay)
                                }
                                _ => None,
                            },
                            (true, false, false) => match key {
                                keyboard::Key::Character(c) => match c.as_str() {
                                    "n" => Some(Message::New),
                                    "e" => Some(Message::ExportButton),
                                    _ => None,
                                },
                                _ => None,
                            },
                            _ => None,
                        }
                    }
                    _ => None,
                },
                Status::Captured => None,
            }),
        ])
    }

    pub fn theme(&self) -> Theme {
        self.theme.clone()
    }
}
