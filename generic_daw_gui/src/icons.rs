macro_rules! icon {
    ($name:ident = $file:expr) => {
        pub static $name: ::std::sync::LazyLock<::iced::widget::svg::Handle> =
            ::std::sync::LazyLock::new(|| {
                ::iced::widget::svg::Handle::from_memory(include_bytes!($file))
            });
    };
}

icon!(ADD = "../../assets/material-symbols--add-2-rounded.svg");
icon!(AUDIO_FILE = "../../assets/material-symbols--audio-file-outline-rounded.svg");
icon!(CHEVRON_RIGHT = "../../assets/material-symbols--chevron-right-rounded.svg");
icon!(GENERIC_FILE = "../../assets/material-symbols--draft-outline-rounded.svg");
icon!(HANDLE = "../../assets/material-symbols--drag-handle-rounded.svg");
icon!(PAUSE = "../../assets/material-symbols--pause-rounded.svg");
icon!(PLAY = "../../assets/material-symbols--play-arrow-rounded.svg");
icon!(RANGE = "../../assets/material-symbols--arrow-range-rounded.svg");
icon!(STOP = "../../assets/material-symbols--stop-rounded.svg");
