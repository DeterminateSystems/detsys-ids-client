use super::{Checkin, coherent_feature_flag_diff::CoherentFlagDiff};

pub(crate) trait CheckinDiff {
    fn diff(&self, prev: Option<&Checkin>) -> Option<String>;
}

impl CheckinDiff for Checkin {
    fn diff(&self, prev: Option<&Checkin>) -> Option<String> {
        let Some(prev) = prev else {
            return Some(String::from(
                "Current is populated, but the previous was None.",
            ));
        };

        if prev == self {
            return None;
        }

        let mut delta: Vec<String> = vec![];

        delta.extend(self.server_options.diff(&prev.server_options));
        delta.extend(self.options.diff(&prev.options));

        Some(delta.join("\n"))
    }
}

impl CheckinDiff for Option<Checkin> {
    fn diff(&self, prev: Option<&Checkin>) -> Option<String> {
        match (self, prev) {
            (None, None) => None,
            (None, Some(_)) => Some(String::from("Current is none, but previous was not.")),
            (Some(s), prev) => s.diff(prev),
        }
    }
}
