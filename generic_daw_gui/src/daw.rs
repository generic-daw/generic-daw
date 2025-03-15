use crate::{
    arrangement_view::{ArrangementView, Message as ArrangementMessage, Tab},
    components::{styled_button, styled_pick_list, styled_scrollable_with_direction, styled_svg},
    file_tree::FileTree,
    icons::{PAUSE, PLAY, RECORD, STOP},
    widget::{BpmInput, LINE_HEIGHT, Redrawer, Strategy, VSplit},
};
use generic_daw_core::{
    Denominator, Meter, Numerator, Stream, VARIANTS as _, build_input_stream,
    clap_host::{self, PluginDescriptor, PluginType, clack_host::bundle::PluginBundle},
};
use hound::WavWriter;
use iced::{
    Alignment::Center,
    Element, Event, Subscription, Task, Theme,
    border::Radius,
    event::{self, Status},
    keyboard,
    widget::{
        button, column, horizontal_space, row,
        scrollable::{Direction, Scrollbar},
        toggler, vertical_space,
    },
    window::{self, Id},
};
use rfd::{AsyncFileDialog, FileHandle};
use std::{
    collections::BTreeMap,
    fs,
    hash::{DefaultHasher, Hash as _, Hasher as _},
    io::BufWriter,
    path::Path,
    sync::{
        Arc, LazyLock,
        atomic::Ordering::{AcqRel, Acquire, Release},
    },
    time::Instant,
};

pub static PLUGINS: LazyLock<BTreeMap<PluginDescriptor, PluginBundle>> =
    LazyLock::new(clap_host::get_installed_plugins);

#[derive(Clone, Debug)]
pub enum Message {
    ThemeChanged(Theme),
    Arrangement(ArrangementMessage),
    FileTree(Box<Path>),
    SamplesFileDialog,
    ExportFileDialog,
    TogglePlay,
    Stop,
    BpmChanged(u16),
    NumeratorChanged(Numerator),
    DenominatorChanged(Denominator),
    ToggleMetronome,
    Tab(Tab),
    SplitAt(f32),
    ToggleRecord,
    RecordingChunk(Box<[f32]>),
    StopRecord,
}

pub struct Daw {
    arrangement: ArrangementView,
    file_tree: FileTree,
    split_at: f32,
    meter: Arc<Meter>,
    theme: Theme,
    recording: Option<(Stream, WavWriter<BufWriter<fs::File>>, Box<Path>)>,
}

impl Daw {
    pub fn create() -> (Self, Task<Message>) {
        let (_, open) = window::open(window::Settings {
            exit_on_close_request: false,
            maximized: true,
            ..window::Settings::default()
        });

        let (arrangement, meter) = ArrangementView::create();

        _ = fs::create_dir(dirs::data_dir().unwrap().join("Generic Daw"));

        (
            Self {
                arrangement,
                file_tree: FileTree::new(&dirs::home_dir().unwrap()),
                split_at: 300.0,
                meter,
                theme: Theme::CatppuccinFrappe,
                recording: None,
            },
            open.discard(),
        )
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::ThemeChanged(theme) => self.theme = theme,
            Message::Arrangement(message) => {
                return self.arrangement.update(message).map(Message::Arrangement);
            }
            Message::FileTree(path) => {
                self.file_tree.update(&path);
            }
            Message::SamplesFileDialog => {
                return Task::future(AsyncFileDialog::new().pick_files()).and_then(|paths| {
                    Task::batch(
                        paths
                            .iter()
                            .map(FileHandle::path)
                            .map(Box::from)
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
                .map(|p| Box::from(p.path()))
                .map(ArrangementMessage::Export)
                .map(Message::Arrangement);
            }
            Message::TogglePlay => {
                if self.meter.playing.fetch_not(AcqRel) {
                    return self.update(Message::StopRecord);
                }
            }
            Message::Stop => {
                self.meter.playing.store(false, Release);
                self.meter.sample.store(0, Release);
                self.arrangement.stop();
                return self.update(Message::StopRecord);
            }
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
            Message::Tab(tab) => self.arrangement.change_tab(tab),
            Message::SplitAt(split_at) => self.split_at = split_at.clamp(100.0, 500.0),
            Message::ToggleRecord => {
                let fut = self.update(Message::Stop);

                let mut file_name = "recording-".to_owned();

                let mut hasher = DefaultHasher::new();
                Instant::now().hash(&mut hasher);
                file_name.push_str(itoa::Buffer::new().format(hasher.finish()));

                file_name.push_str(".wav");

                let path = dirs::data_dir()
                    .unwrap()
                    .join("Generic Daw")
                    .join(file_name)
                    .into();

                let (stream, receiver) = build_input_stream(self.meter.sample_rate);
                let writer = WavWriter::create(
                    &path,
                    hound::WavSpec {
                        channels: 2,
                        sample_rate: self.meter.sample_rate,
                        bits_per_sample: 32,
                        sample_format: hound::SampleFormat::Float,
                    },
                )
                .unwrap();

                self.recording = Some((stream, writer, path));
                self.meter.playing.store(true, Release);

                return fut.chain(Task::stream(receiver).map(Message::RecordingChunk));
            }
            Message::RecordingChunk(samples) => {
                let (_, writer, _) = self.recording.as_mut().unwrap();

                for sample in samples {
                    writer.write_sample(sample).unwrap();
                }
            }
            Message::StopRecord => {
                if let Some((_, writer, path)) = self.recording.take() {
                    writer.finalize().unwrap();
                    return self
                        .arrangement
                        .update(ArrangementMessage::LoadSample(path))
                        .map(Message::Arrangement);
                }
            }
        }

        Task::none()
    }

    pub fn view(&self, window: Id) -> Element<'_, Message> {
        if self.arrangement.clap_host.is_plugin_window(window) {
            return vertical_space().into();
        }

        let playing = self.meter.playing.load(Acquire);

        column![
            row![
                row![
                    styled_button("Load Samples").on_press(Message::SamplesFileDialog),
                    styled_button("Export").on_press(Message::ExportFileDialog),
                ],
                row![
                    Redrawer(playing),
                    styled_button(
                        styled_svg(if playing { PAUSE.clone() } else { PLAY.clone() })
                            .height(LINE_HEIGHT)
                    )
                    .on_press(Message::TogglePlay),
                    styled_button(styled_svg(STOP.clone()).height(LINE_HEIGHT))
                        .on_press(Message::Stop),
                ],
                row![
                    styled_pick_list(
                        Numerator::VARIANTS,
                        Some(self.meter.numerator.load(Acquire)),
                        Message::NumeratorChanged
                    )
                    .width(50),
                    styled_pick_list(
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
                button(styled_svg(RECORD.clone()))
                    .style(|t, s| {
                        let mut style = button::danger(t, s);
                        style.border.radius = Radius::new(f32::INFINITY);
                        style
                    })
                    .padding(3.0)
                    .on_press(Message::ToggleRecord),
                row![
                    styled_button("Arrangement").on_press(Message::Tab(Tab::Arrangement)),
                    styled_button("Mixer").on_press(Message::Tab(Tab::Mixer))
                ],
                horizontal_space(),
                styled_pick_list(Theme::ALL, Some(&self.theme), Message::ThemeChanged),
                styled_pick_list(
                    PLUGINS
                        .keys()
                        .filter(|d| d.ty == PluginType::Instrument)
                        .collect::<Box<[_]>>(),
                    None::<&PluginDescriptor>,
                    |p| Message::Arrangement(ArrangementMessage::LoadInstrumentPlugin(
                        p.to_owned()
                    ))
                )
                .placeholder("Add Instrument")
            ]
            .spacing(20)
            .align_y(Center),
            VSplit::new(
                styled_scrollable_with_direction(
                    self.file_tree.view().0,
                    Direction::Vertical(Scrollbar::default()),
                ),
                self.arrangement.view().map(Message::Arrangement),
            )
            .strategy(Strategy::Left)
            .split_at(self.split_at)
            .on_resize(Message::SplitAt)
        ]
        .padding(20)
        .spacing(20)
        .into()
    }

    pub fn theme(&self, _window: Id) -> Theme {
        self.theme.clone()
    }

    pub fn title(&self, window: Id) -> String {
        self.arrangement
            .clap_host
            .title(window)
            .unwrap_or_else(|| String::from("Generic DAW"))
    }

    pub fn subscription() -> Subscription<Message> {
        Subscription::batch([
            ArrangementView::subscription().map(Message::Arrangement),
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
}
