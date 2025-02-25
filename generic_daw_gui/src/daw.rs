use crate::{
    arrangement_view::{ArrangementView, Message as ArrangementMessage},
    clap_host_view::{ClapHostView, Message as ClapHostMessage},
    widget::{BpmInput, VSplit},
};
use fragile::Fragile;
use generic_daw_core::{
    Denominator, Meter, Numerator, VARIANTS as _,
    clap_host::{self, PluginDescriptor, clack_host::bundle::PluginBundle},
};
use home::home_dir;
use iced::{
    Alignment::Center,
    Element, Event, Length, Subscription, Task, Theme,
    event::{self, Status},
    keyboard,
    widget::{button, column, horizontal_space, pick_list, row, scrollable, svg, toggler},
};
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

const PLAY: &[u8] = include_bytes!("../../assets/material-symbols--play-arrow-rounded.svg");
const PAUSE: &[u8] = include_bytes!("../../assets/material-symbols--pause-rounded.svg");
const STOP: &[u8] = include_bytes!("../../assets/material-symbols--stop-rounded.svg");

#[derive(Clone, Debug)]
pub enum Message {
    ThemeChanged(Theme),
    ClapHost(ClapHostMessage),
    Arrangement(ArrangementMessage),
    LoadPlugin(PluginDescriptor),
    SamplesFileDialog,
    ExportFileDialog,
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
            Message::SamplesFileDialog => {
                return Task::future(AsyncFileDialog::new().pick_files()).and_then(|paths| {
                    Task::batch(
                        paths
                            .iter()
                            .map(FileHandle::path)
                            .map(PathBuf::from)
                            .map(ArrangementMessage::LoadSample)
                            .map(Message::Arrangement)
                            .map(Task::done),
                    )
                });
            }
            Message::ExportFileDialog => {
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
        column![
            row![
                row![
                    button("Load Samples").on_press(Message::SamplesFileDialog),
                    button("Export").on_press(Message::ExportFileDialog),
                    button("New").on_press(Message::New),
                ],
                row![
                    button(
                        svg(svg::Handle::from_memory(
                            if self.meter.playing.load(Acquire) {
                                PAUSE
                            } else {
                                PLAY
                            }
                        ))
                        .style(|theme: &Theme, _| svg::Style {
                            color: Some(theme.extended_palette().secondary.base.text)
                        })
                        .width(Length::Shrink)
                        .height(Length::Fixed(21.0))
                    )
                    .on_press(Message::TogglePlay),
                    button(
                        svg(svg::Handle::from_memory(STOP))
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
                BpmInput::new(self.meter.bpm.load(Acquire), 30..=600, Message::BpmChanged),
                toggler(self.meter.metronome.load(Acquire))
                    .label("Metronome")
                    .on_toggle(|_| Message::ToggleMetronome),
                horizontal_space(),
                pick_list(Theme::ALL, Some(&self.theme), Message::ThemeChanged),
                pick_list(
                    self.plugins.keys().collect::<Box<[_]>>(),
                    None::<&PluginDescriptor>,
                    |p| Message::LoadPlugin(p.to_owned())
                )
                .placeholder("Load Plugin")
                .style(|t: &Theme, s| pick_list::Style {
                    placeholder_color: t.extended_palette().background.weak.text,
                    ..pick_list::default(t, s)
                })
            ]
            .spacing(20)
            .align_y(Center),
            VSplit::new(
                scrollable(file_tree(home_dir().unwrap()).on_double_click(|path| {
                    Message::Arrangement(ArrangementMessage::LoadSample(path))
                }),),
                self.arrangement.view().map(Message::Arrangement)
            )
            .split(0.25)
        ]
        .padding(20)
        .spacing(20)
        .into()
    }

    pub fn subscription() -> Subscription<Message> {
        Subscription::batch([
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
                                    "e" => Some(Message::ExportFileDialog),
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
