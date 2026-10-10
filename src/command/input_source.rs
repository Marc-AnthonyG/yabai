use clap::Subcommand;
use clap::builder::NonEmptyStringValueParser;
use serde::{Deserialize, Serialize};

#[derive(Subcommand, Serialize, Deserialize, Debug)]
pub(crate) enum InputSourceCommand {
    /// Select the next enabled keyboard layout or input mode, wrapping around; with two of them,
    /// it toggles between them
    Cycle,
    /// Select the keyboard layout or input mode ID, such as com.apple.keylayout.Canadian-CSA
    Select {
        #[arg(value_name = "ID", value_parser = NonEmptyStringValueParser::new())]
        input_source_id: String,
    },
}
